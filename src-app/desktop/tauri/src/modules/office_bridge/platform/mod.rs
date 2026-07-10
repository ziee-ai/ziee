//! Cross-platform Office automation seam (ITEM-6).
//!
//! `office_bridge` enumerates + acts on the user's open Word/Excel/PowerPoint
//! documents, installs the bridge cert into the OS trust store, and sideloads
//! the task-pane add-in — all through [`active()`], without the rest of the
//! module knowing *how* the host talks to Office:
//!
//! | Platform                    | mechanism            | gate                        |
//! |-----------------------------|----------------------|-----------------------------|
//! | `WindowsOfficePlatform`     | COM / IDispatch      | `cfg(windows)`              |
//! | `MacOfficePlatform`         | AppleScript / osascript | `cfg(target_os = "macos")` |
//! | `UnsupportedOfficePlatform` | none (self-disables) | everything else             |
//!
//! The seam mirrors `code_sandbox::backend` structurally: a `#[cfg]`-selected
//! `static ACTIVE` reached through `active()`, and a cheap sync `probe()` whose
//! `None` return makes `office_bridge::init()` skip MCP-row registration
//! entirely (headless / Linux servers must not bind 44300 or attempt COM —
//! DEC-3).
//!
//! **Scope of this increment (ITEM-6):** the trait + shared types + probe +
//! the Windows presence probe; every *acting* method is a stub returning a
//! typed "not yet implemented — ITEM-7" error. The real COM / AppleScript
//! implementations land in ITEM-7 / ITEM-8.

use std::path::Path;

use async_trait::async_trait;
use axum::http::StatusCode;
use once_cell::sync::Lazy;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use ziee::AppError;

/// A DER cert staged for a privileged trust-store install, in a freshly-created,
/// private temp directory. Removed on drop.
#[cfg(any(windows, target_os = "macos"))]
pub(crate) struct StagedCert {
    pub path: std::path::PathBuf,
    dir: std::path::PathBuf,
}

#[cfg(any(windows, target_os = "macos"))]
impl Drop for StagedCert {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// Stage `cert_der` in a private, exclusively-created temp directory and return
/// its path. The directory name is unique per invocation and created with
/// `create_dir` (which FAILS if the path already exists — so an attacker cannot
/// pre-place a symlink), and on unix the dir is `0700` and the file `0600`. The
/// cert file itself is opened `create_new` (O_EXCL). Together this closes the
/// predictable-path TOCTOU (CWE-377) on the privileged `security add-trusted-cert`
/// (macOS) / `certutil -addstore Root` (Windows) that reads the staged file.
#[cfg(any(windows, target_os = "macos"))]
pub(crate) fn stage_cert_der(cert_der: &[u8]) -> Result<StagedCert, AppError> {
    use std::io::Write;
    // A high-res nonce (nanos) + a per-process counter + pid make the dir name
    // unique even across a crash+restart with a reused pid, so `create` never
    // trips EEXIST on a stale dir (which would be a fail-safe DoS on cert install).
    static CTR: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let n = CTR.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let mut dir = std::env::temp_dir();
    dir.push(format!("ziee-bridge-cert-{}-{}-{}", std::process::id(), nonce, n));

    let mut builder = std::fs::DirBuilder::new();
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder
        .create(&dir)
        .map_err(|e| AppError::internal_error(format!("stage cert dir: {e}")))?;

    // Own the dir via the RAII guard IMMEDIATELY after creation, so a failure in
    // the open/write below still removes it (no orphaned temp dir on the error path).
    let staged = StagedCert { path: dir.join("bridge.cer"), dir };

    let mut opts = std::fs::OpenOptions::new();
    opts.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(0o600);
    }
    let mut file = opts
        .open(&staged.path)
        .map_err(|e| AppError::internal_error(format!("stage cert file: {e}")))?;
    file.write_all(cert_der)
        .map_err(|e| AppError::internal_error(format!("write temp cert: {e}")))?;

    Ok(staged)
}

#[cfg(windows)]
pub mod windows;
#[cfg(target_os = "macos")]
pub mod macos;
// Fallback for any OS without a native Office automation path. Selected via
// `active()` on non-Windows/non-macOS hosts; kept compiling everywhere the
// same way `code_sandbox::backend` gates its `unsupported` module.
#[cfg(not(any(windows, target_os = "macos")))]
pub mod unsupported;

/// Which Office application a document belongs to.
#[derive(Serialize, Deserialize, JsonSchema, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum OfficeApp {
    Word,
    Excel,
    PowerPoint,
}

/// One currently-open Office document, as enumerated by the platform.
///
/// `full_name` is the app's own fully-qualified document identifier (path +
/// name for a saved doc, or just the name for an unsaved one) — it is the
/// stable handle callers pass back to the pane tools (`read_document`,
/// `run_office_js`, …) to target this document.
#[derive(Serialize, Deserialize, JsonSchema, Clone, Debug)]
pub struct OpenDoc {
    /// The owning application (Word/Excel/PowerPoint).
    pub app: OfficeApp,
    /// Short document name (e.g. `Report.docx`).
    pub name: String,
    /// App-qualified full name — the handle the pane tools target a document by.
    pub full_name: String,
    /// Filesystem path of the containing folder, if the doc has been saved.
    pub path: Option<String>,
    /// Whether the document has no unsaved changes (`Document.Saved`).
    pub saved: bool,
    /// Whether this is the app's currently-active document.
    pub active: bool,
    /// How the platform attached to this doc (e.g. `"com_get_active_object"`,
    /// `"accessible_object_from_window"`, `"enum_windows_presence"`). Purely
    /// diagnostic; opaque to callers.
    pub attach_method: String,
}

/// What the host is capable of, as reported by [`OfficePlatform::probe`].
#[derive(Serialize, Deserialize, JsonSchema, Clone, Copy, Debug)]
pub struct OfficeCaps {
    /// True when the host is a supported *desktop* OS (Windows/macOS) that can
    /// run the bridge + attach to Office. False ⇒ the module self-disables.
    pub desktop: bool,
    /// True when a Microsoft Office installation was detected on the host.
    pub office_present: bool,
}

/// The host-OS-specific Office operations. Selected once at process start via
/// `cfg` and reached through [`active()`].
#[async_trait]
pub trait OfficePlatform: Send + Sync {
    /// One-time boot probe. Cheap, host-only, sync. `None` means "this host is
    /// not a supported desktop OS with Office present" — `office_bridge::init`
    /// then logs the reason and skips MCP-row registration + the bridge
    /// listener entirely (the rest of the server boots fine). `Some(caps)`
    /// carries whether Office was actually detected (a supported desktop with
    /// Office *absent* still registers so the admin `[Connect]` flow can warn).
    fn probe(&self) -> Option<OfficeCaps>;

    /// Enumerate the user's currently-open Word/Excel/PowerPoint documents.
    async fn list_open_documents(&self) -> Result<Vec<OpenDoc>, AppError>;

    /// Install the bridge's self-signed cert (DER bytes) into the OS trust
    /// store so the served `https://localhost` task pane is trusted. May
    /// trigger one elevation prompt (Windows UAC / macOS auth).
    fn install_cert_trust(&self, cert_der: &[u8]) -> Result<(), AppError>;

    /// Register the add-in manifest for sideloading (Windows: HKCU WEF Developer
    /// key; macOS: drop into the per-app `wef` folder).
    fn register_sideload(&self, manifest_path: &Path) -> Result<(), AppError>;

    /// Whether any running Office process is elevated. An elevated Office cannot
    /// be automated from the non-elevated daemon (COM same-integrity rule), so
    /// callers surface a warning. Cheap + sync.
    fn office_is_elevated(&self) -> bool;
}

/// Build the "not yet implemented — ITEM-7" error the ITEM-6 stubs return for
/// every acting method (enumerate / act / cert / sideload). Carries a stable
/// machine-readable code so tests + the MCP dispatch can branch on it.
pub(crate) fn not_implemented_err(what: &str) -> AppError {
    AppError::new(
        StatusCode::NOT_IMPLEMENTED,
        "OFFICE_NOT_IMPLEMENTED",
        format!("{what}: not yet implemented — ITEM-7"),
    )
}

/// Build the "unsupported OS" error the fallback + macOS scaffold return.
pub(crate) fn not_supported_err() -> AppError {
    AppError::new(
        StatusCode::NOT_IMPLEMENTED,
        "OFFICE_PLATFORM_UNSUPPORTED",
        "office bridge is not supported on this operating system",
    )
}

#[cfg(windows)]
static ACTIVE: Lazy<windows::WindowsOfficePlatform> =
    Lazy::new(windows::WindowsOfficePlatform::new);
#[cfg(target_os = "macos")]
static ACTIVE: Lazy<macos::MacOfficePlatform> = Lazy::new(macos::MacOfficePlatform::new);
#[cfg(not(any(windows, target_os = "macos")))]
static ACTIVE: Lazy<unsupported::UnsupportedOfficePlatform> =
    Lazy::new(unsupported::UnsupportedOfficePlatform::new);

/// The Office platform for this host OS. Resolved once via `cfg`.
pub fn active() -> &'static dyn OfficePlatform {
    &*ACTIVE
}

// =====================================================
// Mock implementation (test-only)
// =====================================================

/// In-memory `OfficePlatform` for unit/MCP tests (ITEM-9 injects it to exercise
/// the tool dispatch without a real Office). Probes as a desktop-with-Office and
/// returns canned documents; the acting methods succeed with fixed data.
#[cfg(test)]
pub(crate) struct MockOfficePlatform {
    docs: Vec<OpenDoc>,
    elevated: bool,
    /// Whether `probe()` reports Office as present (default `true`).
    office_present: bool,
    /// Whether `install_cert_trust` succeeds (default `true`); `false` returns a
    /// typed error so TEST-16 can assert `cert_trusted == false`.
    cert_ok: bool,
    /// Whether `register_sideload` succeeds (default `true`); `false` returns a
    /// typed error so TEST-16 can assert `sideloaded == false`.
    sideload_ok: bool,
}

#[cfg(test)]
impl MockOfficePlatform {
    pub(crate) fn new() -> Self {
        Self {
            docs: vec![
                OpenDoc {
                    app: OfficeApp::Word,
                    name: "Report.docx".to_string(),
                    full_name: r"C:\Users\test\Report.docx".to_string(),
                    path: Some(r"C:\Users\test".to_string()),
                    saved: true,
                    active: true,
                    attach_method: "mock".to_string(),
                },
                OpenDoc {
                    app: OfficeApp::Excel,
                    name: "Budget.xlsx".to_string(),
                    full_name: r"C:\Users\test\Budget.xlsx".to_string(),
                    path: Some(r"C:\Users\test".to_string()),
                    saved: false,
                    active: false,
                    attach_method: "mock".to_string(),
                },
            ],
            elevated: false,
            office_present: true,
            cert_ok: true,
            sideload_ok: true,
        }
    }

    /// Seed the mock with an explicit document set (ITEM-9 / TEST-12 injects a
    /// PowerPoint doc to exercise the capability-matrix branch).
    pub(crate) fn with_docs(docs: Vec<OpenDoc>) -> Self {
        Self {
            docs,
            ..Self::new()
        }
    }

    /// Set whether running Office is reported as elevated (TEST-16).
    pub(crate) fn with_elevated(mut self, elevated: bool) -> Self {
        self.elevated = elevated;
        self
    }

    /// Set whether `probe()` reports Office as present (TEST-16).
    pub(crate) fn with_office_present(mut self, office_present: bool) -> Self {
        self.office_present = office_present;
        self
    }

    /// Set whether `install_cert_trust` succeeds (TEST-16).
    pub(crate) fn with_cert_ok(mut self, cert_ok: bool) -> Self {
        self.cert_ok = cert_ok;
        self
    }

    /// Set whether `register_sideload` succeeds (TEST-16).
    pub(crate) fn with_sideload_ok(mut self, sideload_ok: bool) -> Self {
        self.sideload_ok = sideload_ok;
        self
    }
}

#[cfg(test)]
#[async_trait]
impl OfficePlatform for MockOfficePlatform {
    fn probe(&self) -> Option<OfficeCaps> {
        Some(OfficeCaps {
            desktop: true,
            office_present: self.office_present,
        })
    }

    async fn list_open_documents(&self) -> Result<Vec<OpenDoc>, AppError> {
        Ok(self.docs.clone())
    }

    fn install_cert_trust(&self, _cert_der: &[u8]) -> Result<(), AppError> {
        if self.cert_ok {
            Ok(())
        } else {
            Err(AppError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "OFFICE_CERT_TRUST_FAILED",
                "mock: cert trust install failed",
            ))
        }
    }

    fn register_sideload(&self, _manifest_path: &Path) -> Result<(), AppError> {
        if self.sideload_ok {
            Ok(())
        } else {
            Err(AppError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "OFFICE_SIDELOAD_FAILED",
                "mock: sideload registration failed",
            ))
        }
    }

    fn office_is_elevated(&self) -> bool {
        self.elevated
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// TEST-10 — the `OpenDoc.full_name` doc comment (which schemars carries into
    /// the desktop OpenAPI schema + generated `types.ts` JSDoc) no longer references
    /// the removed `act_on_document`, and now points at the pane tools. Guards both
    /// the ITEM-8 reword AND that this doc comment is the schema SOURCE the desktop
    /// `openapi.json`/`types.ts` regen consumes.
    #[test]
    fn test10_open_doc_full_name_desc_has_no_act_on_document() {
        let schema =
            serde_json::to_value(schemars::schema_for!(OpenDoc)).expect("OpenDoc schema serializes");
        let desc = schema["properties"]["full_name"]["description"]
            .as_str()
            .expect("full_name carries a description");
        assert!(
            !desc.contains("act_on_document"),
            "OpenDoc.full_name description must not reference the removed \
             act_on_document: {desc}"
        );
        assert!(
            desc.contains("pane tools"),
            "reworded description should reference the pane tools: {desc}"
        );
    }

    /// TEST-8 — the `#[cfg]`-selected `active()` resolves to a live platform on
    /// every host and the trait is object-safe (usable as `&dyn OfficePlatform`
    /// through both the sync probe path and an async acting method). This is the
    /// seam contract the whole module depends on.
    #[test]
    fn test8_active_returns_object_safe_platform() {
        let platform: &'static dyn OfficePlatform = active();
        // Sync trait-object dispatch (probe + elevation) must be callable.
        let _caps = platform.probe();
        let _elevated = platform.office_is_elevated();
    }

    /// TEST-8 (cont.) — the test-only `MockOfficePlatform` is a valid
    /// `OfficePlatform` returning canned data, so the dispatcher can inject it to
    /// drive MCP dispatch without a real Office install.
    #[tokio::test]
    async fn test8_mock_platform_returns_canned_data() {
        let mock = MockOfficePlatform::new();
        let caps = mock.probe().expect("mock probes as present");
        assert!(caps.desktop && caps.office_present);

        let docs = mock.list_open_documents().await.expect("canned docs");
        assert_eq!(docs.len(), 2);

        // Object-safety through the trait object (what the dispatcher injection uses).
        let as_dyn: &dyn OfficePlatform = &mock;
        assert!(!as_dyn.office_is_elevated());
    }
}
