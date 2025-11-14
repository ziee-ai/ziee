// PDFium utility for runtime usage

use crate::common::AppError;
use pdfium_render::prelude::*;

/// Initialize PDFium library
/// Note: Pdfium is not Send+Sync, so we create a new instance each time
pub fn init_pdfium() -> Result<Pdfium, AppError> {
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
