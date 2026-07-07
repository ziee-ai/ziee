//! macOS `OfficePlatform` scaffold — AppleScript/osascript bridge.
//!
//! **UNVERIFIED — Mac spike (DEC-9).** This box cannot runtime-verify macOS, so
//! every path here is a compiled-but-inert scaffold: the gate is the Keychain
//! cert-trust + WKWebView same-origin-WSS round-trip spike, which has NOT run.
//! `probe()` reports the host as a supported desktop (so the seam + settings
//! surface compile + exercise on macOS) but the acting methods all return the
//! typed unsupported error until ITEM-8 fills in the real AppleScript /
//! `security add-trusted-cert` / `~/Library/Containers/.../wef` implementations.

#![cfg(target_os = "macos")]

use std::path::Path;

use async_trait::async_trait;

use crate::common::AppError;

use super::{ActResult, DocOp, OfficeCaps, OfficePlatform, OpenDoc, not_supported_err};

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

#[async_trait]
impl OfficePlatform for MacOfficePlatform {
    fn probe(&self) -> Option<OfficeCaps> {
        // UNVERIFIED — Mac spike. macOS is a supported desktop shape, but until
        // MAC_TRANSPORT_VERIFIED flips, report Office as absent so nothing
        // downstream assumes a working automation path.
        Some(OfficeCaps {
            desktop: true,
            office_present: false,
        })
    }

    async fn list_open_documents(&self) -> Result<Vec<OpenDoc>, AppError> {
        // UNVERIFIED — Mac spike: osascript/AppleScript enumeration (ITEM-8).
        Err(not_supported_err())
    }

    async fn act_on_document(
        &self,
        _doc_full_name: &str,
        _op: &DocOp,
    ) -> Result<ActResult, AppError> {
        // UNVERIFIED — Mac spike: AppleScript document mutation (ITEM-8).
        Err(not_supported_err())
    }

    fn install_cert_trust(&self, _cert_der: &[u8]) -> Result<(), AppError> {
        // UNVERIFIED — Mac spike: `security add-trusted-cert` Keychain install.
        Err(not_supported_err())
    }

    fn register_sideload(&self, _manifest_path: &Path) -> Result<(), AppError> {
        // UNVERIFIED — Mac spike: drop manifest into
        // ~/Library/Containers/com.microsoft.{Word,Excel,Powerpoint}/Data/Documents/wef.
        Err(not_supported_err())
    }

    fn office_is_elevated(&self) -> bool {
        // UNVERIFIED — Mac spike: no elevation concept mirrored yet.
        false
    }
}
