//! macOS `OfficePlatform` — best-effort AppleScript/`osascript` bridge (ITEM-8).
//!
//! **VERIFIED — 2026-07-08 Mac spike (DEC-9).** Authored on a Windows box, this
//! module was empirically verified end-to-end on a real Apple-Silicon Mac with
//! Office installed (see `MAC_OFFICE_BRIDGE_VERIFICATION.md`). The spike fixed two
//! live osascript bugs (the `repeat with d in (get …)` count error, and an
//! unsaved-doc `save` GUI-dialog hang) and confirmed the transport gate:
//! [`MAC_TRANSPORT_VERIFIED`] is now `true`.
//!
//! The Keychain cert-trust + WKWebView same-origin-WSS round-trip PASSED:
//! `install_cert_trust`'s `security add-trusted-cert` makes the bridge CA trusted,
//! and the Office WKWebView task pane then loads the served `https://localhost`
//! content *prompt-free* with a live WSS connect-back. The still-open item is the
//! 5 pane-mediated Office.js tools (ITEM-9 daemon↔pane RPC), unrelated to transport.
//!
//! Mechanism parity with the Windows COM impl (ITEM-7): enumerate + act via
//! Apple Events (`osascript`), install the cert into the login keychain, and
//! sideload by dropping the manifest into each app's container `wef` folder.
//! Every blocking `osascript`/`security` invocation is fault-isolated per app,
//! and the async trait methods run their blocking work on `spawn_blocking`.

#![cfg(target_os = "macos")]

use std::path::Path;

use async_trait::async_trait;
use axum::http::StatusCode;

use ziee::AppError;

use super::{OfficeApp, OfficeCaps, OfficePlatform, OpenDoc};

/// Whether the macOS transport (Keychain trust + WKWebView same-origin WSS) has
/// been empirically verified. **`true` as of the 2026-07-08 Mac spike (DEC-9)** —
/// see `MAC_OFFICE_BRIDGE_VERIFICATION.md`. On a real Apple-Silicon Mac with Office
/// installed, `security add-trusted-cert` made the minted bridge CA trusted, the
/// Office **WKWebView task pane loaded `https://localhost:44300/taskpane.html`
/// prompt-free**, and its Office.js `wss://localhost:44300/bridge` connect-back +
/// ping/echo round-tripped live (Excel: `bridge open (host=Excel, token=present)`).
///
/// SCOPE: this gates the TRANSPORT (cert trust + WKWebView same-origin WSS) and the
/// osascript tool path (`list_open_documents` + Word `append_paragraph`, both fixed
/// + verified in this spike). It does NOT imply the 5 pane-mediated Office.js tools
/// work — those are the still-unimplemented ITEM-9 daemon↔pane RPC.
pub const MAC_TRANSPORT_VERIFIED: bool = true;

pub struct MacOfficePlatform;

impl MacOfficePlatform {
    pub fn new() -> Self {
        Self
    }
}

impl Default for MacOfficePlatform {
    fn default() -> Self {
        Self::new()
    }
}

// =====================================================
// App table + presence probe
// =====================================================

/// One Office app's macOS coordinates: the `.app` bundle path (presence probe),
/// the AppleScript application display name, and the app's document-collection +
/// active-document keywords (Word/Excel/PowerPoint diverge on these).
struct MacApp {
    app: OfficeApp,
    /// AppleScript application display name (`tell application "<display>"`).
    display: &'static str,
    /// Canonical install path of the app bundle (presence probe).
    bundle_path: &'static str,
    /// The app's document-collection keyword (`documents`/`workbooks`/…).
    collection: &'static str,
    /// The app's active-document reference (`active document`/…).
    active: &'static str,
}

/// The three Office apps, in a stable order. `const` so the presence probe and
/// the enumeration share one source of truth.
const APPS: [MacApp; 3] = [
    MacApp {
        app: OfficeApp::Word,
        display: "Microsoft Word",
        bundle_path: "/Applications/Microsoft Word.app",
        collection: "documents",
        active: "active document",
    },
    MacApp {
        app: OfficeApp::Excel,
        display: "Microsoft Excel",
        bundle_path: "/Applications/Microsoft Excel.app",
        collection: "workbooks",
        active: "active workbook",
    },
    MacApp {
        app: OfficeApp::PowerPoint,
        display: "Microsoft PowerPoint",
        bundle_path: "/Applications/Microsoft PowerPoint.app",
        collection: "presentations",
        active: "active presentation",
    },
];

/// True when any Microsoft Office app bundle is present under `/Applications`.
///
/// UNVERIFIED — Mac spike: filesystem presence probe of the canonical app
/// bundle paths (analogous to the Windows `Program Files` probe).
fn office_present() -> bool {
    APPS.iter().any(|a| Path::new(a.bundle_path).exists())
}

// =====================================================
// AppleScript helpers (pure)
// =====================================================

/// Build the per-app enumeration script. Guarded by `application "..." is
/// running` (a boolean reference property that does NOT launch the app), so an
/// un-launched app contributes nothing rather than being started. Emits one
/// tab-separated `name\tfullName\tsaved\tactive` line per open document.
///
/// Mac spike 2026-07-08 (VERIFIED live against Word+Excel): the enumeration
/// requires `repeat with d in (get {collection})` — a bare `repeat with d in
/// {collection}` raises `-1708` (Word: "every document doesn't understand the
/// count message") / `-50` (Excel: "Parameter error") because AS then tries to
/// `count` the element specifier. The `(get ...)` materializes the list first.
fn list_script(app: &MacApp) -> String {
    format!(
        r#"if application "{display}" is running then
	tell application "{display}"
		set activeName to ""
		try
			set activeName to name of {active}
		end try
		set out to ""
		repeat with d in (get {collection})
			set docName to name of d
			set fullName to docName
			try
				set fullName to full name of d
			end try
			set isSaved to "false"
			try
				set isSaved to (saved of d) as string
			end try
			set isActive to (docName = activeName) as string
			set out to out & docName & tab & fullName & tab & isSaved & tab & isActive & linefeed
		end repeat
		return out
	end tell
end if
return """#,
        display = app.display,
        active = app.active,
        collection = app.collection,
    )
}

/// Parse the tab/linefeed enumeration output of [`list_script`] into `OpenDoc`s,
/// stamping `attach_method = "apple_events"`. Malformed lines are skipped.
fn parse_docs(app: OfficeApp, raw: &str, out: &mut Vec<OpenDoc>) {
    for line in raw.lines() {
        let line = line.trim_end_matches('\r');
        if line.trim().is_empty() {
            continue;
        }
        let mut fields = line.split('\t');
        let name = fields.next().unwrap_or("").to_string();
        if name.is_empty() {
            continue;
        }
        let full_name = fields
            .next()
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
            .unwrap_or_else(|| name.clone());
        let saved = fields
            .next()
            .map(|s| s.eq_ignore_ascii_case("true"))
            .unwrap_or(false);
        let active = fields
            .next()
            .map(|s| s.eq_ignore_ascii_case("true"))
            .unwrap_or(false);
        // Containing folder, if the full name is a path (saved docs).
        let path = Path::new(&full_name).parent().and_then(|p| {
            let s = p.to_string_lossy().to_string();
            if s.is_empty() {
                None
            } else {
                Some(s)
            }
        });
        out.push(OpenDoc {
            app,
            name,
            full_name,
            path,
            saved,
            active,
            attach_method: "apple_events".to_string(),
        });
    }
}

/// Run a single AppleScript via `osascript -e <script>`. `Ok(stdout)` on exit 0,
/// `Err(stderr)` otherwise (an AppleScript `error` yields a non-zero exit).
///
/// UNVERIFIED — Mac spike: `osascript` invocation shape.
fn run_osascript(script: &str) -> Result<String, String> {
    let output = std::process::Command::new("osascript")
        .arg("-e")
        .arg(script)
        .output()
        .map_err(|e| format!("spawn osascript: {e}"))?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).trim().to_string())
    }
}

/// Enumerate every open document across all three apps, fault-isolating each: a
/// failing app is logged + skipped, never aborting the others.
///
/// UNVERIFIED — Mac spike: cross-app enumeration + fault isolation.
fn list_documents_blocking() -> Vec<OpenDoc> {
    let mut out: Vec<OpenDoc> = Vec::new();
    for app in &APPS {
        match run_osascript(&list_script(app)) {
            Ok(raw) => parse_docs(app.app, &raw, &mut out),
            Err(e) => {
                tracing::debug!(
                    "office_bridge(macos): osascript enumerate {} failed (skipping): {e}",
                    app.display
                );
            }
        }
    }
    out
}

// =====================================================
// Trait impl
// =====================================================

#[async_trait]
impl OfficePlatform for MacOfficePlatform {
    fn probe(&self) -> Option<OfficeCaps> {
        // macOS is a supported desktop shape; `office_present` is a real
        // bundle-presence probe. The automation transport is verified as of the
        // 2026-07-08 Mac spike (MAC_TRANSPORT_VERIFIED = true).
        let office_present = office_present();
        if !office_present {
            tracing::info!(
                "office_bridge(macos): no Microsoft Office app bundle found under \
                 /Applications; registering anyway (admin [Connect] will warn)."
            );
        }
        Some(OfficeCaps {
            desktop: true,
            office_present,
        })
    }

    async fn list_open_documents(&self) -> Result<Vec<OpenDoc>, AppError> {
        // UNVERIFIED — Mac spike: osascript/AppleScript enumeration. Blocking
        // `osascript` work runs on a blocking thread; each app is fault-isolated
        // inside, so this returns `Ok(vec)` (possibly empty) rather than failing
        // when one app can't be reached.
        tokio::task::spawn_blocking(list_documents_blocking)
            .await
            .map_err(|e| AppError::internal_error(format!("office_bridge: osascript task join: {e}")))
    }

    fn install_cert_trust(&self, cert_der: &[u8]) -> Result<(), AppError> {
        // VERIFIED — 2026-07-08 Mac spike: stage the DER as a temp `.cer`, then
        // `security add-trusted-cert -d -r trustRoot -k <login.keychain-db>`.
        //
        // This was THE Mac-spike gate, and it PASSED: after this runs, the system
        // SecTrust evaluator (`security verify-cert -p ssl -s localhost`) trusts the
        // leaf, and the Office WKWebView task pane loaded the served
        // `https://localhost:44300` content prompt-free with a live WSS connect-back
        // (see MAC_OFFICE_BRIDGE_VERIFICATION.md). NOTE: `-d` targets the admin trust
        // domain and typically raises ONE GUI admin-auth prompt (the macOS analog of
        // Windows' single UAC on cert install) — acceptable, one-time.
        let mut cert_path = std::env::temp_dir();
        cert_path.push(format!("ziee-bridge-cert-{}.cer", std::process::id()));
        std::fs::write(&cert_path, cert_der)
            .map_err(|e| AppError::internal_error(format!("write temp cert: {e}")))?;

        let home = std::env::var("HOME").unwrap_or_default();
        let keychain = format!("{home}/Library/Keychains/login.keychain-db");

        let outcome = std::process::Command::new("security")
            .arg("add-trusted-cert")
            .arg("-d")
            .arg("-r")
            .arg("trustRoot")
            .arg("-k")
            .arg(&keychain)
            .arg(&cert_path)
            .status();

        // Best-effort cleanup regardless of outcome.
        let _ = std::fs::remove_file(&cert_path);

        match outcome {
            Ok(status) if status.success() => Ok(()),
            Ok(status) => Err(AppError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "OFFICE_CERT_TRUST_FAILED",
                format!("security add-trusted-cert exited with {status}"),
            )),
            Err(e) => Err(AppError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "OFFICE_CERT_TRUST_FAILED",
                format!("spawn security add-trusted-cert: {e}"),
            )),
        }
    }

    fn register_sideload(&self, manifest_path: &Path) -> Result<(), AppError> {
        // UNVERIFIED — Mac spike: copy the manifest into each app's
        // `~/Library/Containers/com.microsoft.{Word,Excel,Powerpoint}/Data/Documents/wef/`
        // (creating dirs as needed). NOTE the lowercase "Powerpoint" container
        // id — it is intentionally NOT "PowerPoint" (the macOS sandbox container
        // uses the lowercase form).
        let home = std::env::var("HOME").map_err(|_| {
            AppError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "OFFICE_SIDELOAD_FAILED",
                "HOME env var is not set",
            )
        })?;
        let file_name = manifest_path.file_name().ok_or_else(|| {
            AppError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "OFFICE_SIDELOAD_FAILED",
                "manifest path has no file name",
            )
        })?;

        // Lowercase "Powerpoint" is deliberate (see the method comment).
        const CONTAINERS: [&str; 3] = [
            "com.microsoft.Word",
            "com.microsoft.Excel",
            "com.microsoft.Powerpoint",
        ];

        let mut succeeded = 0usize;
        let mut last_err = String::new();
        for container in CONTAINERS {
            let wef_dir = Path::new(&home)
                .join("Library/Containers")
                .join(container)
                .join("Data/Documents/wef");
            if let Err(e) = std::fs::create_dir_all(&wef_dir) {
                last_err = format!("{container}: create {}: {e}", wef_dir.display());
                continue;
            }
            let dest = wef_dir.join(file_name);
            match std::fs::copy(manifest_path, &dest) {
                Ok(_) => succeeded += 1,
                Err(e) => last_err = format!("{container}: copy to {}: {e}", dest.display()),
            }
        }

        if succeeded == 0 {
            Err(AppError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "OFFICE_SIDELOAD_FAILED",
                format!("sideload: no Office container was writable ({last_err})"),
            ))
        } else {
            Ok(())
        }
    }

    fn office_is_elevated(&self) -> bool {
        // UNVERIFIED — Mac spike: N/A on macOS. There is no
        // elevation-disables-add-ins issue as on Windows; the macOS analog is
        // TCC Automation (Apple Events) consent, which is a per-app permission
        // prompt, not an integrity-level mismatch. Always `false`.
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// DEC-9: the macOS transport (Keychain trust + WKWebView same-origin WSS) was
    /// empirically verified by the 2026-07-08 Mac spike (see
    /// `MAC_OFFICE_BRIDGE_VERIFICATION.md`), so the gate is now `true`.
    #[test]
    fn mac_transport_verified() {
        assert!(
            MAC_TRANSPORT_VERIFIED,
            "macOS transport verified by the DEC-9 Mac spike"
        );
    }

    /// Enumeration output parsing: tab-separated fields → `OpenDoc`, with the
    /// containing folder derived from a path-shaped `full name`, and unsaved/
    /// nameless-only lines handled.
    #[test]
    fn parse_docs_reads_tab_separated_lines() {
        let raw = "Report.docx\t/Users/x/Report.docx\ttrue\ttrue\n\
                   Untitled\tUntitled\tfalse\tfalse\n\
                   \n";
        let mut out = Vec::new();
        parse_docs(OfficeApp::Word, raw, &mut out);
        assert_eq!(out.len(), 2);

        assert_eq!(out[0].name, "Report.docx");
        assert_eq!(out[0].full_name, "/Users/x/Report.docx");
        assert_eq!(out[0].path.as_deref(), Some("/Users/x"));
        assert!(out[0].saved);
        assert!(out[0].active);
        assert_eq!(out[0].attach_method, "apple_events");

        assert_eq!(out[1].name, "Untitled");
        assert_eq!(out[1].full_name, "Untitled");
        // A bare (unsaved) name has no containing folder.
        assert_eq!(out[1].path, None);
        assert!(!out[1].saved);
        assert!(!out[1].active);
    }
}
