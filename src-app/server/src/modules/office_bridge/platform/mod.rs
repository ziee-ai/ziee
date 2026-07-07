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

use crate::common::AppError;

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
/// stable handle callers pass back to [`OfficePlatform::act_on_document`].
#[derive(Serialize, Deserialize, JsonSchema, Clone, Debug)]
pub struct OpenDoc {
    /// The owning application (Word/Excel/PowerPoint).
    pub app: OfficeApp,
    /// Short document name (e.g. `Report.docx`).
    pub name: String,
    /// App-qualified full name — the handle for `act_on_document`.
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

/// A mutation to apply to a specific open document via
/// [`OfficePlatform::act_on_document`].
///
/// Minimal for ITEM-6 — the full op vocabulary (edit range, add comment,
/// track-changes, …) grows with the MCP tool surface in ITEM-9.
#[derive(Serialize, Deserialize, JsonSchema, Clone, Debug)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum DocOp {
    /// Append a paragraph of text to the end of the document body.
    AppendParagraph { text: String },
}

/// Result of an [`OfficePlatform::act_on_document`] call.
#[derive(Serialize, Deserialize, JsonSchema, Clone, Debug)]
pub struct ActResult {
    /// Whether the operation was applied successfully.
    pub ok: bool,
    /// Optional post-op read-back (e.g. the trailing text after an append) so
    /// callers can confirm the mutation landed. `None` when not read back.
    pub read_back: Option<String>,
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

    /// Apply `op` to the document identified by `doc_full_name` (an app-qualified
    /// `OpenDoc::full_name`).
    async fn act_on_document(
        &self,
        doc_full_name: &str,
        op: &DocOp,
    ) -> Result<ActResult, AppError>;

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
        }
    }

    /// Seed the mock with an explicit document set (ITEM-9 / TEST-12 injects a
    /// PowerPoint doc to exercise the capability-matrix branch).
    pub(crate) fn with_docs(docs: Vec<OpenDoc>) -> Self {
        Self {
            docs,
            elevated: false,
        }
    }
}

#[cfg(test)]
#[async_trait]
impl OfficePlatform for MockOfficePlatform {
    fn probe(&self) -> Option<OfficeCaps> {
        Some(OfficeCaps {
            desktop: true,
            office_present: true,
        })
    }

    async fn list_open_documents(&self) -> Result<Vec<OpenDoc>, AppError> {
        Ok(self.docs.clone())
    }

    async fn act_on_document(
        &self,
        doc_full_name: &str,
        op: &DocOp,
    ) -> Result<ActResult, AppError> {
        let DocOp::AppendParagraph { text } = op;
        // Canned success: echo the appended text back so injecting tests can
        // assert the round-trip reached the (mock) document.
        if self.docs.iter().any(|d| d.full_name == doc_full_name) {
            Ok(ActResult {
                ok: true,
                read_back: Some(text.clone()),
            })
        } else {
            Err(AppError::not_found(doc_full_name))
        }
    }

    fn install_cert_trust(&self, _cert_der: &[u8]) -> Result<(), AppError> {
        Ok(())
    }

    fn register_sideload(&self, _manifest_path: &Path) -> Result<(), AppError> {
        Ok(())
    }

    fn office_is_elevated(&self) -> bool {
        self.elevated
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
    /// `OfficePlatform` returning canned data, so ITEM-9 can inject it to drive
    /// MCP dispatch without a real Office install.
    #[tokio::test]
    async fn test8_mock_platform_returns_canned_data() {
        let mock = MockOfficePlatform::new();
        let caps = mock.probe().expect("mock probes as present");
        assert!(caps.desktop && caps.office_present);

        let docs = mock.list_open_documents().await.expect("canned docs");
        assert_eq!(docs.len(), 2);

        let target = docs[0].full_name.clone();
        let res = mock
            .act_on_document(
                &target,
                &DocOp::AppendParagraph {
                    text: "hello".to_string(),
                },
            )
            .await
            .expect("mock act succeeds");
        assert!(res.ok);
        assert_eq!(res.read_back.as_deref(), Some("hello"));

        // Object-safety through the trait object (what ITEM-9 injection uses).
        let as_dyn: &dyn OfficePlatform = &mock;
        assert!(!as_dyn.office_is_elevated());
    }
}
