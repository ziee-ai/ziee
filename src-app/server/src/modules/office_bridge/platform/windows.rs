//! Windows `OfficePlatform` — COM/IDispatch bridge to open Office documents.
//!
//! **ITEM-6 STUB.** Only [`WindowsOfficePlatform::probe`] is real here (a cheap
//! Office-presence check so `init()` can decide whether to register the module).
//! Every *acting* method — enumerate, act, cert-trust, sideload — returns a
//! typed "not yet implemented — ITEM-7" error. The real COM path
//! (`GetActiveObject` + late-bound `IDispatch` for Word/PowerPoint, oleacc
//! `AccessibleObjectFromWindow` for Excel, `EnumWindows` presence fallback,
//! `certutil` cert install, HKCU WEF sideload, `TokenElevation` checks) lands in
//! ITEM-7 and pulls in the `windows` COM crate — deliberately NOT added yet.

#![cfg(windows)]

use std::path::Path;

use async_trait::async_trait;

use crate::common::AppError;

use super::{
    ActResult, DocOp, OfficeApp, OfficeCaps, OfficePlatform, OpenDoc, not_implemented_err,
};

pub struct WindowsOfficePlatform;

impl WindowsOfficePlatform {
    pub fn new() -> Self {
        Self
    }
}

impl Default for WindowsOfficePlatform {
    fn default() -> Self {
        Self::new()
    }
}

/// Map a top-level Office window class name to its owning application.
///
/// Pure + total (unknown class ⇒ `None`); this is what ITEM-7's `EnumWindows`
/// presence fallback will use to classify a found window without COM. Kept here
/// (rather than in ITEM-7) so the mapping has a home + a regression test now.
pub(crate) fn app_for_window_class(class: &str) -> Option<OfficeApp> {
    match class {
        // Word top-level frame.
        "OpusApp" => Some(OfficeApp::Word),
        // Excel top-level frame.
        "XLMAIN" => Some(OfficeApp::Excel),
        // PowerPoint top-level frame.
        "PPTFrameClass" => Some(OfficeApp::PowerPoint),
        _ => None,
    }
}

/// Detect a Microsoft Office (16.0 / click-to-run or MSI) installation via a
/// cheap filesystem probe of the well-known install roots. A registry check
/// (HKLM/HKCU `...Office\16.0\...\InstallRoot`) would be more thorough but needs
/// `winreg`/`windows-sys` plumbing; a file-existence check is sufficient for the
/// ITEM-6 stub and is the documented acceptable approach. ITEM-7 can tighten it.
fn office_present() -> bool {
    // Resolve the Program Files roots from the environment (robust to a
    // non-`C:` system drive / localized folder), falling back to the literal
    // defaults. Both the 64-bit and 32-bit roots are checked because Office's
    // bitness is independent of the OS bitness.
    let mut roots: Vec<String> = Vec::new();
    for var in ["ProgramFiles", "ProgramFiles(x86)", "ProgramW6432"] {
        if let Ok(val) = std::env::var(var) {
            if !val.is_empty() {
                roots.push(val);
            }
        }
    }
    if roots.is_empty() {
        roots.push(r"C:\Program Files".to_string());
        roots.push(r"C:\Program Files (x86)".to_string());
    }

    // Click-to-run layout: <root>\Microsoft Office\root\Office16\EXCEL.EXE.
    // MSI layout: <root>\Microsoft Office\Office16\EXCEL.EXE. Any Office app
    // binary confirms presence; EXCEL/WINWORD/POWERPNT cover the trio.
    const RELATIVE: [&str; 6] = [
        r"Microsoft Office\root\Office16\EXCEL.EXE",
        r"Microsoft Office\root\Office16\WINWORD.EXE",
        r"Microsoft Office\root\Office16\POWERPNT.EXE",
        r"Microsoft Office\Office16\EXCEL.EXE",
        r"Microsoft Office\Office16\WINWORD.EXE",
        r"Microsoft Office\Office16\POWERPNT.EXE",
    ];
    for root in &roots {
        for rel in RELATIVE {
            if Path::new(root).join(rel).exists() {
                return true;
            }
        }
    }
    false
}

#[async_trait]
impl OfficePlatform for WindowsOfficePlatform {
    fn probe(&self) -> Option<OfficeCaps> {
        // Windows is always a supported desktop for the bridge; report whether
        // Office was actually detected so the admin `[Connect]` flow can warn
        // when the desktop is supported but Office is absent (still registers).
        let office_present = office_present();
        if !office_present {
            tracing::info!(
                "office_bridge: Windows desktop detected but no Office 16.0 \
                 install found; registering anyway (admin [Connect] will warn)."
            );
        }
        Some(OfficeCaps {
            desktop: true,
            office_present,
        })
    }

    async fn list_open_documents(&self) -> Result<Vec<OpenDoc>, AppError> {
        Err(not_implemented_err("list_open_documents (Windows COM)"))
    }

    async fn act_on_document(
        &self,
        _doc_full_name: &str,
        _op: &DocOp,
    ) -> Result<ActResult, AppError> {
        Err(not_implemented_err("act_on_document (Windows COM)"))
    }

    fn install_cert_trust(&self, _cert_der: &[u8]) -> Result<(), AppError> {
        Err(not_implemented_err("install_cert_trust (certutil -addstore)"))
    }

    fn register_sideload(&self, _manifest_path: &Path) -> Result<(), AppError> {
        Err(not_implemented_err("register_sideload (HKCU WEF Developer)"))
    }

    fn office_is_elevated(&self) -> bool {
        // STUB: the real check enumerates Office pids and reads `TokenElevation`
        // via the `windows` crate (ITEM-7). Until then, report not-elevated so
        // the module never *falsely* blocks on an elevation warning; the true
        // check is additive (a warning), not load-bearing for correctness.
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// TEST-10 — the pure window-class → `OfficeApp` mapping ITEM-7's
    /// `EnumWindows` presence fallback relies on. Total + case-sensitive (the
    /// real Win32 class names are exact); unknown classes must map to `None`
    /// rather than mis-classifying a stray window as an Office document.
    #[test]
    fn test10_app_for_window_class_maps_known_office_frames() {
        assert_eq!(app_for_window_class("OpusApp"), Some(OfficeApp::Word));
        assert_eq!(app_for_window_class("XLMAIN"), Some(OfficeApp::Excel));
        assert_eq!(
            app_for_window_class("PPTFrameClass"),
            Some(OfficeApp::PowerPoint)
        );
        // Unknown / wrong-case must not classify.
        assert_eq!(app_for_window_class("Notepad"), None);
        assert_eq!(app_for_window_class("opusapp"), None);
        assert_eq!(app_for_window_class(""), None);
    }
}
