// PDFium utility for runtime usage

use std::cell::RefCell;

use crate::common::AppError;
use pdfium_render::prelude::*;

thread_local! {
    /// Per-thread cached `Pdfium`. `Pdfium` (and its library bindings) are
    /// `!Send`, so a thread-local — not a global `OnceLock` — is the correct
    /// cache. All PDF work runs inside `tokio::task::spawn_blocking`, so each
    /// blocking-pool worker binds the dynamic library exactly once and then
    /// reuses it, instead of re-`dlopen`-ing on every PDF fetch.
    static PDFIUM: RefCell<Option<Pdfium>> = const { RefCell::new(None) };
}

/// Run `f` with a thread-local, lazily-initialized `Pdfium` instance.
///
/// The closure receives `&Pdfium`; any `PdfDocument`/`PdfPage` it loads borrows
/// from that reference and must not escape the closure (the same lifetime
/// constraint the old `init_pdfium()` imposed by ownership). Binding the
/// library is the expensive part and now happens once per worker thread.
pub fn with_pdfium<R>(f: impl FnOnce(&Pdfium) -> Result<R, AppError>) -> Result<R, AppError> {
    PDFIUM.with(|cell| {
        // Initialize on first use for this thread.
        if cell.borrow().is_none() {
            let pdfium = build_pdfium()?;
            *cell.borrow_mut() = Some(pdfium);
        }
        let slot = cell.borrow();
        f(slot.as_ref().expect("pdfium initialized above"))
    })
}

/// Bind the PDFium dynamic library (embedded first, then system fallback) and
/// construct a fresh `Pdfium`. Callers should prefer [`with_pdfium`], which
/// caches the result per thread; this builder is invoked once per worker.
fn build_pdfium() -> Result<Pdfium, AppError> {
    // Try embedded library first (extracted to app_data_dir/bin/)
    match super::embedded::get_pdfium_path() {
        Ok(library_path) => {
            tracing::debug!("Using embedded PDFium at {:?}", library_path);

            // Get directory containing the library
            let lib_dir = library_path.parent().unwrap().to_str().unwrap();

            // Try to bind to the embedded library
            match Pdfium::bind_to_library(Pdfium::pdfium_platform_library_name_at_path(lib_dir)) {
                Ok(bindings) => return Ok(Pdfium::new(bindings)),
                Err(e) => {
                    tracing::warn!("Failed to bind to embedded PDFium: {}, trying system", e);
                }
            }
        }
        Err(e) => {
            tracing::warn!("Failed to get embedded PDFium: {}, trying system", e);
        }
    }

    // Fallback to system library
    let bindings = Pdfium::bind_to_system_library()
        .map_err(|e| AppError::internal_error(format!("Failed to bind to PDFium library: {}", e)))?;

    Ok(Pdfium::new(bindings))
}
