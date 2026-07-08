//! Windows `OfficePlatform` — COM/IDispatch bridge to open Office documents.
//!
//! **ITEM-7.** The real COM path: `GetActiveObject` + late-bound `IDispatch`
//! for Word/PowerPoint, oleacc `AccessibleObjectFromWindow` on the
//! `XLMAIN ▸ XLDESK ▸ EXCEL7` child for Excel, an `EnumWindows` presence
//! fallback (`OpusApp/XLMAIN/PPTFrameClass`), `act_on_document`
//! (Content.InsertAfter + Save + read-back), `certutil -addstore -f Root`
//! cert install via an elevated `ShellExecuteExW` (one UAC), HKCU WEF
//! sideload registration, and `TokenElevation` checks on the Office pids.
//!
//! Every `unsafe` COM call is confined to this file. The blocking COM work runs
//! inside `tokio::task::spawn_blocking`, each on a thread that initializes its
//! own STA apartment (`CoInitializeEx(APARTMENTTHREADED)`) via [`ComGuard`] and
//! tears it down on drop. `VARIANT` lifetime is managed by the `windows` crate
//! itself (its `Drop` calls `VariantClear`, its `Clone` calls `VariantCopy`),
//! and `BSTR` frees itself — so the late-binding helpers below stay leak-free
//! without hand-rolled `SysFreeString`/`VariantClear` calls.

#![cfg(windows)]

use std::ffi::c_void;
use std::path::Path;

use async_trait::async_trait;
use axum::http::StatusCode;

use windows::Win32::Foundation::{CloseHandle, HANDLE, HWND, LPARAM};
use windows::Win32::Security::{
    GetTokenInformation, TOKEN_ELEVATION, TOKEN_QUERY, TokenElevation,
};
use windows::Win32::System::Com::{
    CLSIDFromProgID, COINIT_APARTMENTTHREADED, CoInitializeEx, CoUninitialize, DISPATCH_FLAGS,
    DISPATCH_METHOD, DISPATCH_PROPERTYGET, DISPPARAMS, IDispatch,
};
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW, TH32CS_SNAPPROCESS,
};
use windows::Win32::System::Ole::GetActiveObject;
use windows::Win32::System::Registry::{
    HKEY, HKEY_CURRENT_USER, KEY_WRITE, REG_OPTION_NON_VOLATILE, REG_SZ, RegCloseKey,
    RegCreateKeyExW, RegSetValueExW,
};
use windows::Win32::System::Threading::{
    GetExitCodeProcess, OpenProcess, OpenProcessToken, PROCESS_QUERY_LIMITED_INFORMATION,
    WaitForSingleObject,
};
use windows::Win32::System::Variant::{VARIANT, VT_BSTR};
use windows::Win32::UI::Accessibility::AccessibleObjectFromWindow;
use windows::Win32::UI::Shell::{SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW, ShellExecuteExW};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, FindWindowExW, GetClassNameW, GetWindowTextW, IsWindowVisible, SW_HIDE,
};
use windows::core::{BSTR, GUID, IUnknown, Interface, PCWSTR, Result as WinResult};

use ziee::AppError;

use super::{ActResult, DocOp, OfficeApp, OfficeCaps, OfficePlatform, OpenDoc};

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

// =====================================================
// Constants
// =====================================================

/// `OBJID_NATIVEOM` — the object id `AccessibleObjectFromWindow` uses to hand
/// back the Excel window's native OLE Automation `IDispatch` (`Window` object)
/// rather than an MSAA accessibility wrapper.
const OBJID_NATIVEOM: u32 = 0xFFFF_FFF0;

/// `LOCALE_USER_DEFAULT` — the LCID passed to `GetIDsOfNames`/`Invoke`.
const LOCALE_USER_DEFAULT: u32 = 0x0400;

/// The three Office executables checked for elevation.
const OFFICE_EXES: [&str; 3] = ["WINWORD.EXE", "EXCEL.EXE", "POWERPNT.EXE"];

/// `IID_IDispatch` — the interface requested from `AccessibleObjectFromWindow`.
fn iid_idispatch() -> GUID {
    GUID::from_u128(0x00020400_0000_0000_C000_000000000046)
}

/// Map a top-level Office window class name to its owning application.
///
/// Pure + total (unknown class ⇒ `None`); this is what the `EnumWindows`
/// presence fallback uses to classify a found window without COM. Kept here so
/// the mapping has a home + a regression test (TEST-10).
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
/// cheap filesystem probe of the well-known install roots.
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

// =====================================================
// COM apartment guard
// =====================================================

/// Per-thread STA apartment guard. Initializes COM (`APARTMENTTHREADED`) on
/// construction and uninitializes it on drop — but only when *this* call is the
/// one that transitioned the thread into the apartment (a `RPC_E_CHANGED_MODE`
/// return means someone else owns the apartment, so we must NOT uninit).
struct ComGuard {
    should_uninit: bool,
}

impl ComGuard {
    fn new() -> Self {
        // SAFETY: no reserved pointer, valid apartment flag. Idempotent per
        // thread; `S_OK`/`S_FALSE` both mean this thread is in the STA now.
        let hr = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
        Self {
            should_uninit: hr.is_ok(),
        }
    }
}

impl Drop for ComGuard {
    fn drop(&mut self) {
        if self.should_uninit {
            // SAFETY: balances the `CoInitializeEx` that returned S_OK/S_FALSE.
            unsafe { CoUninitialize() };
        }
    }
}

// =====================================================
// Wide-string + VARIANT helpers
// =====================================================

/// UTF-16, NUL-terminated. The returned `Vec` must outlive any `PCWSTR` taken
/// over it.
fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Pull a `String` out of a `VT_BSTR` VARIANT (the type Office returns for
/// Name/FullName/Path/Range.Text). Any other variant type yields `None`.
///
/// SAFETY: reads the VARIANT union; caller guarantees `v` is a live VARIANT.
unsafe fn variant_to_string(v: &VARIANT) -> Option<String> {
    unsafe {
        if v.Anonymous.Anonymous.vt == VT_BSTR {
            // Deref the `ManuallyDrop<BSTR>` in place; the BSTR is owned by the
            // VARIANT and freed when the VARIANT drops, so we only borrow it.
            let bstr: &BSTR = &v.Anonymous.Anonymous.Anonymous.bstrVal;
            Some(bstr.to_string())
        } else {
            None
        }
    }
}

/// Coerce a VARIANT to `bool` (Office `Saved` is `VARIANT_BOOL` in Word/Excel,
/// an `MsoTriState` int in PowerPoint — `VariantToBoolean` handles both).
fn variant_to_bool(v: &VARIANT) -> bool {
    bool::try_from(v).unwrap_or(false)
}

/// Coerce a VARIANT to `i32` (collection `.Count`).
fn variant_to_i32(v: &VARIANT) -> i32 {
    i32::try_from(v).unwrap_or(0)
}

/// Extract the `IDispatch` a property/method returned (a sub-object like
/// `Documents`, `ActiveDocument`, `Content`, `Range`).
fn variant_to_dispatch(v: &VARIANT) -> WinResult<IDispatch> {
    IDispatch::try_from(v)
}

// =====================================================
// IDispatch late binding
// =====================================================

/// The core late-binding call: resolve `name` to a DISPID via `GetIDsOfNames`,
/// then `Invoke` it with `flags` and (reversed, per the IDispatch ABI) `args`.
/// Returns the result VARIANT (VT_EMPTY when the member returns nothing).
///
/// SAFETY: `disp` must be a live `IDispatch`; runs inside a `ComGuard` apartment.
unsafe fn com_invoke(
    disp: &IDispatch,
    name: &str,
    args: &[VARIANT],
    flags: DISPATCH_FLAGS,
) -> WinResult<VARIANT> {
    unsafe {
        let wname = wide(name);
        let name_ptr = PCWSTR(wname.as_ptr());
        let mut dispid: i32 = 0;
        disp.GetIDsOfNames(
            &GUID::zeroed(),
            &name_ptr,
            1,
            LOCALE_USER_DEFAULT,
            &mut dispid,
        )?;

        // IDispatch expects positional args in REVERSE order. `VARIANT: Clone`
        // is `VariantCopy` (deep), so both these copies and the caller's
        // originals free independently — no double-free, no leak.
        let mut reversed: Vec<VARIANT> = args.iter().rev().cloned().collect();
        let params = DISPPARAMS {
            rgvarg: if reversed.is_empty() {
                std::ptr::null_mut()
            } else {
                reversed.as_mut_ptr()
            },
            rgdispidNamedArgs: std::ptr::null_mut(),
            cArgs: reversed.len() as u32,
            cNamedArgs: 0,
        };

        let mut result = VARIANT::default();
        disp.Invoke(
            dispid,
            &GUID::zeroed(),
            LOCALE_USER_DEFAULT,
            flags,
            &params,
            Some(&mut result),
            None,
            None,
        )?;
        Ok(result)
    }
}

/// `DISPATCH_PROPERTYGET` a named property (`Name`, `FullName`, `Count`, …).
unsafe fn com_get(disp: &IDispatch, name: &str) -> WinResult<VARIANT> {
    unsafe { com_invoke(disp, name, &[], DISPATCH_PROPERTYGET) }
}

/// `Item(index)` on a 1-based Office collection. Uses `METHOD|PROPERTYGET`
/// because collection default members are ambiguous across the Office object
/// models.
unsafe fn com_item(disp: &IDispatch, index: i32) -> WinResult<IDispatch> {
    unsafe {
        let flags = DISPATCH_FLAGS(DISPATCH_METHOD.0 | DISPATCH_PROPERTYGET.0);
        let v = com_invoke(disp, "Item", &[VARIANT::from(index)], flags)?;
        variant_to_dispatch(&v)
    }
}

/// `DISPATCH_METHOD`-invoke a method (`InsertAfter`, `Save`).
unsafe fn com_call(disp: &IDispatch, name: &str, args: &[VARIANT]) -> WinResult<VARIANT> {
    unsafe { com_invoke(disp, name, args, DISPATCH_METHOD) }
}

/// `com_get` then coerce to a sub-`IDispatch` in one step.
unsafe fn com_get_dispatch(disp: &IDispatch, name: &str) -> WinResult<IDispatch> {
    unsafe { variant_to_dispatch(&com_get(disp, name)?) }
}

/// `com_get` then coerce to `String` (or `None`).
unsafe fn com_get_string(disp: &IDispatch, name: &str) -> Option<String> {
    unsafe { com_get(disp, name).ok().and_then(|v| variant_to_string(&v)) }
}

// =====================================================
// Attach helpers
// =====================================================

/// `CLSIDFromProgID` → `GetActiveObject` → QI `IDispatch`. `Err` means the app
/// isn't running / isn't registered in the ROT for this session/integrity.
///
/// SAFETY: runs inside a `ComGuard` apartment.
unsafe fn get_active_dispatch(progid: &str) -> WinResult<IDispatch> {
    unsafe {
        let wp = wide(progid);
        let clsid = CLSIDFromProgID(PCWSTR(wp.as_ptr()))?;
        let mut unk: Option<IUnknown> = None;
        GetActiveObject(&clsid, None, &mut unk)?;
        match unk {
            Some(u) => u.cast::<IDispatch>(),
            // E_FAIL — GetActiveObject returned success with a null pointer.
            None => Err(windows::core::Error::from_hresult(windows::core::HRESULT(
                0x8000_4005u32 as i32,
            ))),
        }
    }
}

/// Excel-specific attach: from an `XLMAIN` frame HWND, walk `XLDESK ▸ EXCEL7`
/// and pull the native OM `IDispatch` (the `Window` object) via
/// `AccessibleObjectFromWindow(OBJID_NATIVEOM, IID_IDispatch)`.
///
/// SAFETY: `hwnd_raw` is a live top-level HWND; runs inside a `ComGuard`.
unsafe fn excel_window_dispatch(hwnd_raw: isize) -> Option<IDispatch> {
    unsafe {
        let xlmain = HWND(hwnd_raw as *mut c_void);
        let desk_cls = wide("XLDESK");
        let desk = FindWindowExW(Some(xlmain), None, PCWSTR(desk_cls.as_ptr()), PCWSTR::null())
            .ok()?;
        if desk.0.is_null() {
            return None;
        }
        let e7_cls = wide("EXCEL7");
        let excel7 = FindWindowExW(Some(desk), None, PCWSTR(e7_cls.as_ptr()), PCWSTR::null())
            .ok()?;
        if excel7.0.is_null() {
            return None;
        }
        let iid = iid_idispatch();
        let mut raw: *mut c_void = std::ptr::null_mut();
        AccessibleObjectFromWindow(excel7, OBJID_NATIVEOM, &iid, &mut raw).ok()?;
        if raw.is_null() {
            return None;
        }
        // `AccessibleObjectFromWindow` hands back an already-AddRef'd interface;
        // `from_raw` takes ownership so the `IDispatch`'s Drop releases it.
        Some(IDispatch::from_raw(raw))
    }
}

// =====================================================
// Window enumeration
// =====================================================

/// A visible Office frame window: (raw HWND, class name, window title).
struct OfficeWindow {
    hwnd: isize,
    class: String,
    title: String,
}

struct EnumCollector {
    windows: Vec<OfficeWindow>,
}

/// `EnumWindows` callback: record visible top-level windows whose class is one
/// of the three Office frames. `param1` is a `*mut EnumCollector`.
unsafe extern "system" fn enum_windows_proc(hwnd: HWND, lparam: LPARAM) -> windows::core::BOOL {
    unsafe {
        let collector = &mut *(lparam.0 as *mut EnumCollector);
        if !IsWindowVisible(hwnd).as_bool() {
            return windows::core::BOOL(1);
        }
        let mut class_buf = [0u16; 256];
        let clen = GetClassNameW(hwnd, &mut class_buf);
        if clen <= 0 {
            return windows::core::BOOL(1);
        }
        let class = String::from_utf16_lossy(&class_buf[..clen as usize]);
        if app_for_window_class(&class).is_some() {
            let mut title_buf = [0u16; 512];
            let tlen = GetWindowTextW(hwnd, &mut title_buf);
            let title = if tlen > 0 {
                String::from_utf16_lossy(&title_buf[..tlen as usize])
            } else {
                String::new()
            };
            collector.windows.push(OfficeWindow {
                hwnd: hwnd.0 as isize,
                class,
                title,
            });
        }
        windows::core::BOOL(1)
    }
}

/// Enumerate the visible Word/Excel/PowerPoint top-level frames.
fn enumerate_office_windows() -> Vec<OfficeWindow> {
    let mut collector = EnumCollector {
        windows: Vec::new(),
    };
    // SAFETY: the collector outlives the (synchronous) EnumWindows call.
    unsafe {
        let _ = EnumWindows(
            Some(enum_windows_proc),
            LPARAM(&mut collector as *mut _ as isize),
        );
    }
    collector.windows
}

// =====================================================
// Per-app document collection
// =====================================================

/// Read `Name`/`FullName`/`Path`/`Saved` off one document/workbook/presentation
/// `IDispatch` into an `OpenDoc`. `active_name` is the app's active-doc name
/// (matched by `Name`); `attach_method` is stamped through.
unsafe fn read_doc(
    app: OfficeApp,
    doc: &IDispatch,
    active_name: Option<&str>,
    attach_method: &str,
) -> OpenDoc {
    unsafe {
        let name = com_get_string(doc, "Name").unwrap_or_default();
        let full_name = com_get_string(doc, "FullName").unwrap_or_else(|| name.clone());
        let path_raw = com_get_string(doc, "Path").unwrap_or_default();
        let saved = com_get(doc, "Saved")
            .ok()
            .map(|v| variant_to_bool(&v))
            .unwrap_or(false);
        let path = if path_raw.is_empty() {
            None
        } else {
            Some(path_raw)
        };
        let active = active_name == Some(name.as_str());
        OpenDoc {
            app,
            name,
            full_name,
            path,
            saved,
            active,
            attach_method: attach_method.to_string(),
        }
    }
}

/// Iterate a 1-based `collection` (`Documents`/`Workbooks`/`Presentations`),
/// pushing one `OpenDoc` per item. One failed item never aborts the rest.
unsafe fn collect_from_collection(
    out: &mut Vec<OpenDoc>,
    app: OfficeApp,
    collection: &IDispatch,
    active_name: Option<&str>,
    attach_method: &str,
) {
    unsafe {
        let count = com_get(collection, "Count")
            .map(|v| variant_to_i32(&v))
            .unwrap_or(0);
        for i in 1..=count {
            if let Ok(doc) = com_item(collection, i) {
                out.push(read_doc(app, &doc, active_name, attach_method));
            }
        }
    }
}

/// Word: `GetActiveObject` → `Documents` (+ `ActiveDocument`).
unsafe fn collect_word(out: &mut Vec<OpenDoc>) {
    unsafe {
        let Ok(app) = get_active_dispatch("Word.Application") else {
            return;
        };
        let Ok(documents) = com_get_dispatch(&app, "Documents") else {
            return;
        };
        let active_name = com_get_dispatch(&app, "ActiveDocument")
            .ok()
            .and_then(|d| com_get_string(&d, "Name"));
        collect_from_collection(
            out,
            OfficeApp::Word,
            &documents,
            active_name.as_deref(),
            "com_get_active_object",
        );
    }
}

/// PowerPoint: `GetActiveObject` → `Presentations` (+ `ActivePresentation`).
unsafe fn collect_powerpoint(out: &mut Vec<OpenDoc>) {
    unsafe {
        let Ok(app) = get_active_dispatch("PowerPoint.Application") else {
            return;
        };
        let Ok(presentations) = com_get_dispatch(&app, "Presentations") else {
            return;
        };
        let active_name = com_get_dispatch(&app, "ActivePresentation")
            .ok()
            .and_then(|d| com_get_string(&d, "Name"));
        collect_from_collection(
            out,
            OfficeApp::PowerPoint,
            &presentations,
            active_name.as_deref(),
            "com_get_active_object",
        );
    }
}

/// Excel: `GetActiveObject` is unreliable for Excel (ROT quirk), so fall back to
/// `AccessibleObjectFromWindow` on each `XLMAIN ▸ XLDESK ▸ EXCEL7` → `.Application`.
unsafe fn collect_excel(out: &mut Vec<OpenDoc>, windows: &[OfficeWindow]) {
    unsafe {
        let mut attach_method = "com_get_active_object";
        let mut app = get_active_dispatch("Excel.Application").ok();
        if app.is_none() {
            attach_method = "accessible_object_from_window";
            for w in windows.iter().filter(|w| w.class == "XLMAIN") {
                if let Some(window_disp) = excel_window_dispatch(w.hwnd) {
                    if let Ok(a) = com_get_dispatch(&window_disp, "Application") {
                        app = Some(a);
                        break;
                    }
                }
            }
        }
        let Some(app) = app else {
            return;
        };
        let Ok(workbooks) = com_get_dispatch(&app, "Workbooks") else {
            return;
        };
        let active_name = com_get_dispatch(&app, "ActiveWorkbook")
            .ok()
            .and_then(|d| com_get_string(&d, "Name"));
        collect_from_collection(
            out,
            OfficeApp::Excel,
            &workbooks,
            active_name.as_deref(),
            attach_method,
        );
    }
}

/// For any app that produced NO COM documents but has a visible frame, add a
/// presence-only `OpenDoc` from the window title (`window_enum_presence`). COM
/// results are authoritative, so an app with ≥1 COM doc is skipped entirely.
fn add_window_enum_fallback(out: &mut Vec<OpenDoc>, windows: &[OfficeWindow]) {
    let com_apps: Vec<OfficeApp> = out.iter().map(|d| d.app).collect();
    for w in windows {
        let Some(app) = app_for_window_class(&w.class) else {
            continue;
        };
        if com_apps.contains(&app) {
            continue;
        }
        let name = w.title.trim().to_string();
        if name.is_empty() {
            continue;
        }
        if out.iter().any(|d| d.app == app && d.name == name) {
            continue;
        }
        out.push(OpenDoc {
            app,
            name: name.clone(),
            full_name: name,
            path: None,
            // Presence-only: we couldn't read the real save state via COM.
            saved: true,
            active: false,
            attach_method: "window_enum_presence".to_string(),
        });
    }
}

/// The full enumeration, run on a COM-initialized blocking thread. Each app is
/// wrapped so a single failing app never aborts the others.
fn list_documents_blocking() -> Vec<OpenDoc> {
    let _com = ComGuard::new();
    let windows = enumerate_office_windows();
    let mut out: Vec<OpenDoc> = Vec::new();
    // SAFETY: all COM calls run within the `_com` apartment on this thread.
    unsafe {
        collect_word(&mut out);
        collect_excel(&mut out, &windows);
        collect_powerpoint(&mut out);
    }
    add_window_enum_fallback(&mut out, &windows);
    out
}

/// Word `act_on_document`, run on a COM-initialized blocking thread. Finds the
/// doc whose `FullName` (or `Name`) matches, appends a paragraph, saves, and
/// reads back the last paragraph's text.
fn act_word_blocking(target_full_name: &str, text: &str) -> Result<ActResult, String> {
    let _com = ComGuard::new();
    // SAFETY: all COM calls run within the `_com` apartment on this thread.
    unsafe {
        let app =
            get_active_dispatch("Word.Application").map_err(|e| format!("Word not attachable: {e}"))?;
        let documents =
            com_get_dispatch(&app, "Documents").map_err(|e| format!("Documents: {e}"))?;
        let count = com_get(&documents, "Count")
            .map(|v| variant_to_i32(&v))
            .unwrap_or(0);

        let mut target: Option<IDispatch> = None;
        for i in 1..=count {
            if let Ok(doc) = com_item(&documents, i) {
                let full = com_get_string(&doc, "FullName").unwrap_or_default();
                let name = com_get_string(&doc, "Name").unwrap_or_default();
                if full == target_full_name || name == target_full_name {
                    target = Some(doc);
                    break;
                }
            }
        }
        let doc = target
            .ok_or_else(|| format!("no open Word document matches '{target_full_name}'"))?;

        // Content.InsertAfter("\r" + text) — the spike-proven append.
        let content = com_get_dispatch(&doc, "Content").map_err(|e| format!("Content: {e}"))?;
        let arg = VARIANT::from(format!("\r{text}").as_str());
        com_call(&content, "InsertAfter", &[arg]).map_err(|e| format!("InsertAfter: {e}"))?;
        com_call(&doc, "Save", &[]).map_err(|e| format!("Save: {e}"))?;

        // Read back the last paragraph's text (Paragraphs.Item(Count).Range.Text).
        let read_back = com_get_dispatch(&doc, "Paragraphs").ok().and_then(|paragraphs| {
            let pcount = com_get(&paragraphs, "Count")
                .map(|v| variant_to_i32(&v))
                .unwrap_or(0);
            if pcount < 1 {
                return None;
            }
            com_item(&paragraphs, pcount)
                .ok()
                .and_then(|para| com_get_dispatch(&para, "Range").ok())
                .and_then(|range| com_get_string(&range, "Text"))
                .map(|s| s.trim().to_string())
        });

        Ok(ActResult {
            ok: true,
            read_back,
        })
    }
}

// =====================================================
// Elevation check
// =====================================================

/// Read the NUL-terminated exe name out of a `PROCESSENTRY32W::szExeFile`.
fn exe_name(buf: &[u16]) -> String {
    let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    String::from_utf16_lossy(&buf[..end])
}

/// `OpenProcess` + `OpenProcessToken` + `GetTokenInformation(TokenElevation)`
/// for one pid. `false` on any failure (e.g. a higher-integrity process we
/// can't open — which itself implies it is NOT automatable, but we report the
/// conservative "not elevated" so the warning is only raised on a token we
/// could actually read as elevated).
unsafe fn process_is_elevated(pid: u32) -> bool {
    unsafe {
        let Ok(process) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) else {
            return false;
        };
        let mut token = HANDLE::default();
        let mut elevated = false;
        if OpenProcessToken(process, TOKEN_QUERY, &mut token).is_ok() {
            let mut elevation = TOKEN_ELEVATION {
                TokenIsElevated: 0,
            };
            let mut ret_len: u32 = 0;
            let size = std::mem::size_of::<TOKEN_ELEVATION>() as u32;
            if GetTokenInformation(
                token,
                TokenElevation,
                Some(&mut elevation as *mut _ as *mut c_void),
                size,
                &mut ret_len,
            )
            .is_ok()
            {
                elevated = elevation.TokenIsElevated != 0;
            }
            let _ = CloseHandle(token);
        }
        let _ = CloseHandle(process);
        elevated
    }
}

/// Enumerate running processes; return `true` if any WINWORD/EXCEL/POWERPNT is
/// elevated.
fn office_is_elevated_blocking() -> bool {
    // SAFETY: snapshot handle closed before return; entries are stack-local.
    unsafe {
        let Ok(snapshot) = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) else {
            return false;
        };
        let mut entry = PROCESSENTRY32W::default();
        entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
        let mut elevated = false;
        if Process32FirstW(snapshot, &mut entry).is_ok() {
            loop {
                let name = exe_name(&entry.szExeFile);
                if OFFICE_EXES.iter().any(|e| e.eq_ignore_ascii_case(&name))
                    && process_is_elevated(entry.th32ProcessID)
                {
                    elevated = true;
                    break;
                }
                if Process32NextW(snapshot, &mut entry).is_err() {
                    break;
                }
            }
        }
        let _ = CloseHandle(snapshot);
        elevated
    }
}

// =====================================================
// Trait impl
// =====================================================

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
        // COM is blocking + apartment-bound → run on a dedicated blocking thread
        // that owns its own STA. Each app is fault-isolated inside; a panic
        // (should never happen) surfaces as an internal error rather than
        // poisoning the runtime.
        tokio::task::spawn_blocking(list_documents_blocking)
            .await
            .map_err(|e| AppError::internal_error(format!("office_bridge: COM task join: {e}")))
    }

    async fn act_on_document(
        &self,
        doc_full_name: &str,
        op: &DocOp,
    ) -> Result<ActResult, AppError> {
        let DocOp::AppendParagraph { text } = op;
        let target = doc_full_name.to_string();
        let text = text.clone();
        let res = tokio::task::spawn_blocking(move || act_word_blocking(&target, &text))
            .await
            .map_err(|e| AppError::internal_error(format!("office_bridge: COM task join: {e}")))?;
        res.map_err(|msg| {
            AppError::new(StatusCode::INTERNAL_SERVER_ERROR, "OFFICE_ACT_FAILED", msg)
        })
    }

    fn install_cert_trust(&self, cert_der: &[u8]) -> Result<(), AppError> {
        // Stage the DER as a temp `.cer`, then `certutil -addstore -f Root` it
        // via an elevated ShellExecute (one UAC). Wait for completion so the
        // caller can report success/failure, then remove the temp file.
        let mut cert_path = std::env::temp_dir();
        cert_path.push(format!("ziee-bridge-cert-{}.cer", std::process::id()));
        std::fs::write(&cert_path, cert_der)
            .map_err(|e| AppError::internal_error(format!("write temp cert: {e}")))?;

        let outcome = run_elevated_certutil(&cert_path);
        let _ = std::fs::remove_file(&cert_path);
        outcome.map_err(|msg| {
            AppError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "OFFICE_CERT_TRUST_FAILED",
                msg,
            )
        })
    }

    fn register_sideload(&self, manifest_path: &Path) -> Result<(), AppError> {
        let path_str = manifest_path.to_string_lossy().to_string();
        // SAFETY: HKCU write; key handle closed before return.
        unsafe {
            let subkey = wide(r"Software\Microsoft\Office\16.0\WEF\Developer");
            let mut hkey = HKEY::default();
            let rc = RegCreateKeyExW(
                HKEY_CURRENT_USER,
                PCWSTR(subkey.as_ptr()),
                None,
                PCWSTR::null(),
                REG_OPTION_NON_VOLATILE,
                KEY_WRITE,
                None,
                &mut hkey,
                None,
            );
            if rc.0 != 0 {
                return Err(AppError::new(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "OFFICE_SIDELOAD_FAILED",
                    format!("RegCreateKeyExW(HKCU WEF Developer) failed: {}", rc.0),
                ));
            }

            // WEF Developer convention: value NAME == value DATA == manifest path
            // (REG_SZ). The data is the wide bytes INCLUDING the NUL terminator.
            let name = wide(&path_str);
            let data = wide(&path_str);
            let data_bytes =
                std::slice::from_raw_parts(data.as_ptr() as *const u8, data.len() * 2);
            let sc = RegSetValueExW(hkey, PCWSTR(name.as_ptr()), None, REG_SZ, Some(data_bytes));
            let _ = RegCloseKey(hkey);
            if sc.0 != 0 {
                return Err(AppError::new(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "OFFICE_SIDELOAD_FAILED",
                    format!("RegSetValueExW(manifest) failed: {}", sc.0),
                ));
            }
        }
        Ok(())
    }

    fn office_is_elevated(&self) -> bool {
        office_is_elevated_blocking()
    }
}

/// `ShellExecuteExW("runas", "certutil", "-addstore -f Root <cer>")`, waited to
/// completion. `Ok(())` iff certutil exits 0.
fn run_elevated_certutil(cert_path: &Path) -> Result<(), String> {
    // SAFETY: all pointers reference wide buffers kept alive for the call; the
    // process handle from SEE_MASK_NOCLOSEPROCESS is closed before return.
    unsafe {
        let verb = wide("runas");
        let file = wide("certutil");
        let params = wide(&format!("-addstore -f Root \"{}\"", cert_path.display()));

        let mut info = SHELLEXECUTEINFOW {
            cbSize: std::mem::size_of::<SHELLEXECUTEINFOW>() as u32,
            fMask: SEE_MASK_NOCLOSEPROCESS,
            lpVerb: PCWSTR(verb.as_ptr()),
            lpFile: PCWSTR(file.as_ptr()),
            lpParameters: PCWSTR(params.as_ptr()),
            nShow: SW_HIDE.0,
            ..Default::default()
        };

        ShellExecuteExW(&mut info)
            .map_err(|e| format!("ShellExecuteExW(runas certutil): {e}"))?;
        if info.hProcess.0.is_null() {
            return Err("ShellExecuteExW returned no process handle".to_string());
        }

        // certutil is quick; cap the wait at 120s to avoid hanging the caller.
        WaitForSingleObject(info.hProcess, 120_000);
        let mut code: u32 = 1;
        let _ = GetExitCodeProcess(info.hProcess, &mut code);
        let _ = CloseHandle(info.hProcess);
        if code == 0 {
            Ok(())
        } else {
            Err(format!("certutil exited with code {code}"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// TEST-10 — the pure window-class → `OfficeApp` mapping the `EnumWindows`
    /// presence fallback relies on. Total + case-sensitive (the real Win32 class
    /// names are exact); unknown classes must map to `None` rather than
    /// mis-classifying a stray window as an Office document.
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

    /// TEST-10 (cont.) — the Excel child-walk targets the exact class chain the
    /// spike proved (`XLMAIN ▸ XLDESK ▸ EXCEL7`) and the native-OM object id.
    /// These constants are load-bearing: a typo silently breaks Excel attach.
    #[test]
    fn test10_excel_child_walk_targets_are_correct() {
        // OBJID_NATIVEOM is the documented native-OM accessibility object id.
        assert_eq!(OBJID_NATIVEOM, 0xFFFF_FFF0);
        // IID_IDispatch is the well-known {00020400-...} interface id.
        assert_eq!(
            iid_idispatch(),
            GUID::from_u128(0x00020400_0000_0000_C000_000000000046)
        );
        // The frame classes the Excel walk keys off must map as expected.
        assert_eq!(app_for_window_class("XLMAIN"), Some(OfficeApp::Excel));
    }
}
