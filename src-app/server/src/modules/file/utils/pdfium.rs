// PDFium utility for runtime usage

use std::sync::OnceLock;
use std::sync::mpsc::{Sender, channel};

use crate::common::AppError;
use pdfium_render::prelude::*;

/// A unit of PDF work to run on the dedicated PDFium thread. Receives the
/// single process-wide `&Pdfium` and is responsible for sending its own result
/// back to the caller.
type Job = Box<dyn FnOnce(&Pdfium) + Send>;

/// Sender to the single PDFium worker thread. `Pdfium` (pdfium-render) is
/// `!Send`/`!Sync` and `FPDF_InitLibrary`/`FPDF_DestroyLibrary` are process-wide
/// global state, so it is unsound to hold multiple `Pdfium` instances across
/// threads (a `Drop` on one tears down the library for all). We therefore own
/// exactly ONE `Pdfium`, built once on a dedicated long-lived thread, and
/// funnel every PDF operation to it over this channel. This both fixes the
/// per-fetch re-`dlopen`/re-init cost and removes the multi-init hazard.
static WORKER: OnceLock<Sender<Job>> = OnceLock::new();

fn worker() -> &'static Sender<Job> {
    WORKER.get_or_init(|| {
        let (tx, rx) = channel::<Job>();
        std::thread::Builder::new()
            .name("pdfium-worker".to_string())
            .spawn(move || {
                // Bind + init PDFium exactly once on this thread. The instance
                // lives for the whole process, so the library is never
                // destroyed-then-used by another thread.
                let pdfium = match build_pdfium() {
                    Ok(p) => p,
                    Err(e) => {
                        tracing::error!(
                            "pdfium worker: initialization failed ({e}); PDF features disabled"
                        );
                        // Returning drops `rx`; queued + future `with_pdfium`
                        // sends then fail, surfacing a clean error per call.
                        return;
                    }
                };
                while let Ok(job) = rx.recv() {
                    job(&pdfium);
                }
            })
            .expect("failed to spawn pdfium worker thread");
        tx
    })
}

/// Run `f` on the single PDFium worker thread and block until it returns.
///
/// The closure receives `&Pdfium`; any `PdfDocument`/`PdfPage` it loads borrows
/// from that reference and must not escape the closure. Because this blocks on
/// the worker, **async callers must invoke it inside `tokio::task::spawn_blocking`**
/// (synchronous CPU-bound PDF paths already do).
pub fn with_pdfium<R, F>(f: F) -> Result<R, AppError>
where
    F: FnOnce(&Pdfium) -> Result<R, AppError> + Send + 'static,
    R: Send + 'static,
{
    let (rtx, rrx) = channel::<Result<R, AppError>>();
    let job: Job = Box::new(move |pdfium| {
        // Ignore send error: it only fails if the caller already gave up.
        let _ = rtx.send(f(pdfium));
    });
    worker()
        .send(job)
        .map_err(|_| AppError::internal_error("PDFium worker unavailable (initialization failed)"))?;
    rrx.recv()
        .map_err(|_| AppError::internal_error("PDFium worker dropped the job"))?
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
