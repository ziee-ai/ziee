//! macOS `OfficePlatform` — best-effort AppleScript/`osascript` bridge (ITEM-8).
//!
//! **UNVERIFIED — Mac spike (DEC-9).** This entire module is a best-effort,
//! std-only scaffold authored on a Windows box. It is `#[cfg(target_os =
//! "macos")]`, so it is NOT compiled or runtime-verified here — every method
//! body is written in plain `std` (`std::process::Command`, `std::fs`,
//! `std::path`) against the crate's shared trait types so it is as
//! syntactically safe as possible and *plausibly* compiles on macOS, but no
//! claim is made that any path actually works.
//!
//! The gate remains [`MAC_TRANSPORT_VERIFIED`] (`false`): the Keychain
//! cert-trust + WKWebView same-origin-WSS round-trip spike has NOT run. The
//! `install_cert_trust` path in particular runs `security add-trusted-cert`,
//! but whether a WKWebView task pane then trusts that cert *prompt-free* is THE
//! unproven Mac-spike unknown — see the comment on that method.
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

use super::{ActResult, DocOp, OfficeApp, OfficeCaps, OfficePlatform, OpenDoc};

/// Whether the macOS transport (Keychain trust + WKWebView same-origin WSS) has
/// been empirically verified. Hard `false` until the Mac spike runs (DEC-9); no
/// code path should claim macOS works while this is false.
pub const MAC_TRANSPORT_VERIFIED: bool = false;

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

/// Escape a Rust string for embedding inside an AppleScript double-quoted
/// literal: backslash and double-quote only (the two chars AS treats specially
/// inside `"..."`). Embedded newlines are left as-is — a raw newline inside an
/// AS string literal is an edge case not handled here (UNVERIFIED — Mac spike).
fn applescript_escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

/// Build the per-app enumeration script. Guarded by `application "..." is
/// running` (a boolean reference property that does NOT launch the app), so an
/// un-launched app contributes nothing rather than being started. Emits one
/// tab-separated `name\tfullName\tsaved\tactive` line per open document.
///
/// UNVERIFIED — Mac spike: AppleScript enumeration shape per app.
fn list_script(app: &MacApp) -> String {
    format!(
        r#"if application "{display}" is running then
	tell application "{display}"
		set activeName to ""
		try
			set activeName to name of {active}
		end try
		set out to ""
		repeat with d in {collection}
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

/// Word-only `AppendParagraph`: find the doc whose `full name`/`name` matches,
/// append a paragraph to the body text, save, and read back the last paragraph.
/// Mirrors the Windows Word-only append (ITEM-7); a target belonging to
/// Excel/PowerPoint simply won't match and errors.
///
/// UNVERIFIED — Mac spike: AppleScript document mutation + read-back.
fn act_word_blocking(target: &str, text: &str) -> Result<ActResult, String> {
    let target_esc = applescript_escape(target);
    let text_esc = applescript_escape(text);
    let script = format!(
        r#"if application "Microsoft Word" is running then
	tell application "Microsoft Word"
		set theDoc to missing value
		repeat with d in documents
			try
				if (full name of d is "{target}") or (name of d is "{target}") then
					set theDoc to d
				end if
			end try
		end repeat
		if theDoc is missing value then error "no open Word document matches"
		set textObj to text object of theDoc
		set content of textObj to (content of textObj) & return & "{text}"
		save theDoc
		set lastPara to "{text}"
		try
			set lastPara to content of text object of (last paragraph of textObj)
		end try
		return lastPara
	end tell
else
	error "Microsoft Word is not running"
end if"#,
        target = target_esc,
        text = text_esc,
    );
    let raw = run_osascript(&script)?;
    let trimmed = raw.trim().to_string();
    let read_back = if trimmed.is_empty() {
        None
    } else {
        Some(trimmed)
    };
    Ok(ActResult {
        ok: true,
        read_back,
    })
}

// =====================================================
// Trait impl
// =====================================================

#[async_trait]
impl OfficePlatform for MacOfficePlatform {
    fn probe(&self) -> Option<OfficeCaps> {
        // UNVERIFIED — Mac spike: macOS is a supported desktop shape, so the
        // seam + settings surface exercise here; `office_present` is a real
        // bundle-presence probe. The automation *transport* is still gated by
        // MAC_TRANSPORT_VERIFIED (false) — nothing downstream should treat a
        // `Some(..)` here as proof the AppleScript path works.
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

    async fn act_on_document(
        &self,
        doc_full_name: &str,
        op: &DocOp,
    ) -> Result<ActResult, AppError> {
        // UNVERIFIED — Mac spike: AppleScript append-paragraph + save + read-back
        // (Word only, mirroring the Windows ITEM-7 op).
        let DocOp::AppendParagraph { text } = op;
        let target = doc_full_name.to_string();
        let text = text.clone();
        let res = tokio::task::spawn_blocking(move || act_word_blocking(&target, &text))
            .await
            .map_err(|e| {
                AppError::internal_error(format!("office_bridge: osascript task join: {e}"))
            })?;
        res.map_err(|msg| {
            AppError::new(StatusCode::INTERNAL_SERVER_ERROR, "OFFICE_ACT_FAILED", msg)
        })
    }

    fn install_cert_trust(&self, cert_der: &[u8]) -> Result<(), AppError> {
        // UNVERIFIED — Mac spike: stage the DER as a temp `.cer`, then
        // `security add-trusted-cert -d -r trustRoot -k <login.keychain-db>`.
        //
        // **THIS IS THE MAC SPIKE GATE.** `security add-trusted-cert` typically
        // prompts for auth, and — critically — it is UNVERIFIED that a WKWebView
        // task pane then trusts the served `https://localhost` cert *prompt-free*
        // (the WKWebView / ATS trust store may not honor a user-added trustRoot
        // the way Safari/Keychain does). Until that round-trip is proven,
        // MAC_TRANSPORT_VERIFIED stays false regardless of this returning `Ok`.
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

    /// DEC-9 guard: the macOS transport is never claimed as verified. (Compiled
    /// only on macOS; will not run on the Windows authoring box.)
    #[test]
    fn mac_transport_stays_unverified() {
        assert!(
            !MAC_TRANSPORT_VERIFIED,
            "macOS transport must stay UNVERIFIED until the Mac spike runs"
        );
    }

    /// AppleScript string escaping handles the two chars special inside a
    /// double-quoted AS literal.
    #[test]
    fn applescript_escape_escapes_quote_and_backslash() {
        assert_eq!(applescript_escape(r#"a"b"#), r#"a\"b"#);
        assert_eq!(applescript_escape(r"a\b"), r"a\\b");
        assert_eq!(applescript_escape("plain"), "plain");
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
