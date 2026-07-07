//! Fallback `OfficePlatform` for any OS without a native Office automation path
//! (headless Linux servers, BSD, …). Keeps the module compiling everywhere and
//! makes it *self-disable*: `probe()` returns `None`, so `office_bridge::init`
//! logs the reason and never registers the MCP row or binds the bridge (DEC-3).
//! Every acting method returns a typed unsupported error.

use std::path::Path;

use async_trait::async_trait;

use crate::common::AppError;

use super::{ActResult, DocOp, OfficeCaps, OfficePlatform, OpenDoc, not_supported_err};

pub struct UnsupportedOfficePlatform;

impl UnsupportedOfficePlatform {
    pub fn new() -> Self {
        Self
    }
}

impl Default for UnsupportedOfficePlatform {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl OfficePlatform for UnsupportedOfficePlatform {
    fn probe(&self) -> Option<OfficeCaps> {
        tracing::info!(
            "office_bridge: OS {} has no Office automation backend; the \
             office_bridge MCP row will NOT be registered.",
            std::env::consts::OS
        );
        None
    }

    async fn list_open_documents(&self) -> Result<Vec<OpenDoc>, AppError> {
        Err(not_supported_err())
    }

    async fn act_on_document(
        &self,
        _doc_full_name: &str,
        _op: &DocOp,
    ) -> Result<ActResult, AppError> {
        Err(not_supported_err())
    }

    fn install_cert_trust(&self, _cert_der: &[u8]) -> Result<(), AppError> {
        Err(not_supported_err())
    }

    fn register_sideload(&self, _manifest_path: &Path) -> Result<(), AppError> {
        Err(not_supported_err())
    }

    fn office_is_elevated(&self) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// TEST-11 — the fallback truly self-disables: `probe()` is `None` (so
    /// `init()` skips registration) and every acting method returns the typed
    /// `OFFICE_PLATFORM_UNSUPPORTED` error rather than panicking or succeeding.
    #[tokio::test]
    async fn test11_unsupported_probe_none_and_methods_error() {
        let platform = UnsupportedOfficePlatform::new();
        assert!(
            platform.probe().is_none(),
            "unsupported host must probe None so init() skips registration"
        );
        assert!(!platform.office_is_elevated());

        let list_err = platform.list_open_documents().await.unwrap_err();
        assert_eq!(list_err.error_code(), "OFFICE_PLATFORM_UNSUPPORTED");

        let act_err = platform
            .act_on_document(
                "whatever",
                &DocOp::AppendParagraph {
                    text: "x".to_string(),
                },
            )
            .await
            .unwrap_err();
        assert_eq!(act_err.error_code(), "OFFICE_PLATFORM_UNSUPPORTED");

        assert_eq!(
            platform.install_cert_trust(&[0u8]).unwrap_err().error_code(),
            "OFFICE_PLATFORM_UNSUPPORTED"
        );
        assert_eq!(
            platform
                .register_sideload(Path::new("manifest.xml"))
                .unwrap_err()
                .error_code(),
            "OFFICE_PLATFORM_UNSUPPORTED"
        );
    }

    /// DEC-9 guard: the macOS transport is never claimed as verified. Compiled
    /// only on macOS (where `platform::macos` exists).
    #[cfg(target_os = "macos")]
    #[test]
    fn test11_mac_transport_unverified() {
        assert!(
            !super::super::macos::MAC_TRANSPORT_VERIFIED,
            "macOS transport must stay UNVERIFIED until the Mac spike runs"
        );
    }
}
