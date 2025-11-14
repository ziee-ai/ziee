// Embedded binaries for Pandoc and PDFium

use crate::common::AppError;
use once_cell::sync::OnceCell;
use std::path::PathBuf;

// Helper macro to embed files with compression
macro_rules! embed_compressed {
    ($path:expr) => {{
        const BYTES: &[u8] = include_bytes!($path);
        BYTES
    }};
}

// Platform-specific embedded binaries
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
mod binaries {
    pub const PANDOC: &[u8] = embed_compressed!(concat!(env!("CARGO_MANIFEST_DIR"), "/binaries/x86_64-unknown-linux-gnu/pandoc/pandoc"));
    pub const PDFIUM: &[u8] = embed_compressed!(concat!(env!("CARGO_MANIFEST_DIR"), "/binaries/x86_64-unknown-linux-gnu/pdfium/libpdfium.so"));
    pub const PANDOC_NAME: &str = "pandoc";
    pub const PDFIUM_NAME: &str = "libpdfium.so";
}

#[cfg(all(target_os = "macos", target_arch = "x86_64"))]
mod binaries {
    pub const PANDOC: &[u8] = embed_compressed!(concat!(env!("CARGO_MANIFEST_DIR"), "/binaries/x86_64-apple-darwin/pandoc/pandoc"));
    pub const PDFIUM: &[u8] = embed_compressed!(concat!(env!("CARGO_MANIFEST_DIR"), "/binaries/x86_64-apple-darwin/pdfium/libpdfium.dylib"));
    pub const PANDOC_NAME: &str = "pandoc";
    pub const PDFIUM_NAME: &str = "libpdfium.dylib";
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
mod binaries {
    pub const PANDOC: &[u8] = embed_compressed!(concat!(env!("CARGO_MANIFEST_DIR"), "/binaries/aarch64-apple-darwin/pandoc/pandoc"));
    pub const PDFIUM: &[u8] = embed_compressed!(concat!(env!("CARGO_MANIFEST_DIR"), "/binaries/aarch64-apple-darwin/pdfium/libpdfium.dylib"));
    pub const PANDOC_NAME: &str = "pandoc";
    pub const PDFIUM_NAME: &str = "libpdfium.dylib";
}

#[cfg(all(target_os = "windows", target_arch = "x86_64"))]
mod binaries {
    pub const PANDOC: &[u8] = embed_compressed!(concat!(env!("CARGO_MANIFEST_DIR"), "/binaries/x86_64-pc-windows-msvc/pandoc/pandoc.exe"));
    pub const PDFIUM: &[u8] = embed_compressed!(concat!(env!("CARGO_MANIFEST_DIR"), "/binaries/x86_64-pc-windows-msvc/pdfium/pdfium.dll"));
    pub const PANDOC_NAME: &str = "pandoc.exe";
    pub const PDFIUM_NAME: &str = "pdfium.dll";
}

#[cfg(not(any(
    all(target_os = "linux", target_arch = "x86_64"),
    all(target_os = "macos", target_arch = "x86_64"),
    all(target_os = "macos", target_arch = "aarch64"),
    all(target_os = "windows", target_arch = "x86_64")
)))]
compile_error!("Unsupported platform - binaries not embedded for this target");

static EXTRACTED_PATHS: OnceCell<ExtractedPaths> = OnceCell::new();

struct ExtractedPaths {
    pandoc: PathBuf,
    pdfium: PathBuf,
}

/// Extract embedded binaries on first call only
/// Extracts to app_data_dir/bin/
pub fn ensure_binaries_extracted() -> Result<(), AppError> {
    EXTRACTED_PATHS.get_or_try_init(|| -> Result<ExtractedPaths, AppError> {
        let app_data_dir = crate::core::get_app_data_dir();
        let bin_dir = app_data_dir.join("bin");
        std::fs::create_dir_all(&bin_dir)
            .map_err(|e| AppError::internal_error(format!("Failed to create bin dir: {}", e)))?;

        let pandoc_path = bin_dir.join(binaries::PANDOC_NAME);
        let pdfium_path = bin_dir.join(binaries::PDFIUM_NAME);

        // Extract Pandoc if not exists
        if !pandoc_path.exists() {
            tracing::info!("Extracting embedded Pandoc to {:?}", pandoc_path);
            std::fs::write(&pandoc_path, binaries::PANDOC)
                .map_err(|e| AppError::internal_error(format!("Failed to extract Pandoc: {}", e)))?;

            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let mut perms = std::fs::metadata(&pandoc_path)
                    .map_err(|e| AppError::internal_error(format!("Failed to get Pandoc permissions: {}", e)))?
                    .permissions();
                perms.set_mode(0o755);
                std::fs::set_permissions(&pandoc_path, perms)
                    .map_err(|e| AppError::internal_error(format!("Failed to set Pandoc permissions: {}", e)))?;
            }

            tracing::info!("Successfully extracted Pandoc ({} bytes)", binaries::PANDOC.len());
        } else {
            tracing::debug!("Pandoc already extracted at {:?}", pandoc_path);
        }

        // Extract PDFium if not exists
        if !pdfium_path.exists() {
            tracing::info!("Extracting embedded PDFium to {:?}", pdfium_path);
            std::fs::write(&pdfium_path, binaries::PDFIUM)
                .map_err(|e| AppError::internal_error(format!("Failed to extract PDFium: {}", e)))?;

            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let mut perms = std::fs::metadata(&pdfium_path)
                    .map_err(|e| AppError::internal_error(format!("Failed to get PDFium permissions: {}", e)))?
                    .permissions();
                perms.set_mode(0o755);
                std::fs::set_permissions(&pdfium_path, perms)
                    .map_err(|e| AppError::internal_error(format!("Failed to set PDFium permissions: {}", e)))?;
            }

            tracing::info!("Successfully extracted PDFium ({} bytes)", binaries::PDFIUM.len());
        } else {
            tracing::debug!("PDFium already extracted at {:?}", pdfium_path);
        }

        Ok(ExtractedPaths {
            pandoc: pandoc_path,
            pdfium: pdfium_path,
        })
    })?;
    Ok(())
}

fn get_extracted_paths() -> Result<&'static ExtractedPaths, AppError> {
    EXTRACTED_PATHS.get().ok_or_else(|| AppError::internal_error("Binaries not extracted yet"))
}

pub fn get_pandoc_path() -> Result<&'static PathBuf, AppError> {
    Ok(&get_extracted_paths()?.pandoc)
}

pub fn get_pdfium_path() -> Result<&'static PathBuf, AppError> {
    Ok(&get_extracted_paths()?.pdfium)
}
