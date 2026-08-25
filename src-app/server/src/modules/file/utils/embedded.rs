// Embedded binaries for Pandoc, typst, and PDFium

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
    pub const TYPST: &[u8] = embed_compressed!(concat!(env!("CARGO_MANIFEST_DIR"), "/binaries/x86_64-unknown-linux-gnu/typst/typst"));
    pub const PANDOC_NAME: &str = "pandoc";
    pub const PDFIUM_NAME: &str = "libpdfium.so";
    pub const TYPST_NAME: &str = "typst";
}

#[cfg(all(target_os = "macos", target_arch = "x86_64"))]
mod binaries {
    pub const PANDOC: &[u8] = embed_compressed!(concat!(env!("CARGO_MANIFEST_DIR"), "/binaries/x86_64-apple-darwin/pandoc/pandoc"));
    pub const PDFIUM: &[u8] = embed_compressed!(concat!(env!("CARGO_MANIFEST_DIR"), "/binaries/x86_64-apple-darwin/pdfium/libpdfium.dylib"));
    pub const TYPST: &[u8] = embed_compressed!(concat!(env!("CARGO_MANIFEST_DIR"), "/binaries/x86_64-apple-darwin/typst/typst"));
    pub const PANDOC_NAME: &str = "pandoc";
    pub const PDFIUM_NAME: &str = "libpdfium.dylib";
    pub const TYPST_NAME: &str = "typst";
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
mod binaries {
    pub const PANDOC: &[u8] = embed_compressed!(concat!(env!("CARGO_MANIFEST_DIR"), "/binaries/aarch64-apple-darwin/pandoc/pandoc"));
    pub const PDFIUM: &[u8] = embed_compressed!(concat!(env!("CARGO_MANIFEST_DIR"), "/binaries/aarch64-apple-darwin/pdfium/libpdfium.dylib"));
    pub const TYPST: &[u8] = embed_compressed!(concat!(env!("CARGO_MANIFEST_DIR"), "/binaries/aarch64-apple-darwin/typst/typst"));
    pub const PANDOC_NAME: &str = "pandoc";
    pub const PDFIUM_NAME: &str = "libpdfium.dylib";
    pub const TYPST_NAME: &str = "typst";
}

#[cfg(all(target_os = "windows", target_arch = "x86_64"))]
mod binaries {
    pub const PANDOC: &[u8] = embed_compressed!(concat!(env!("CARGO_MANIFEST_DIR"), "/binaries/x86_64-pc-windows-msvc/pandoc/pandoc.exe"));
    pub const PDFIUM: &[u8] = embed_compressed!(concat!(env!("CARGO_MANIFEST_DIR"), "/binaries/x86_64-pc-windows-msvc/pdfium/pdfium.dll"));
    pub const TYPST: &[u8] = embed_compressed!(concat!(env!("CARGO_MANIFEST_DIR"), "/binaries/x86_64-pc-windows-msvc/typst/typst.exe"));
    pub const PANDOC_NAME: &str = "pandoc.exe";
    pub const PDFIUM_NAME: &str = "pdfium.dll";
    pub const TYPST_NAME: &str = "typst.exe";
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
    typst: PathBuf,
}

/// Write embedded bytes to `target` if not already on disk (intact) and set the
/// executable bit on Unix. Delegates to the shared cross-process-atomic extractor
/// (`common::embedded::extract_atomic`: temp-write + `fs::rename` + advisory
/// flock) so two concurrent server processes sharing `~/.ziee/bin` never produce
/// a torn binary.
fn extract_one(label: &str, bytes: &[u8], target: &PathBuf) -> Result<(), AppError> {
    crate::common::embedded::extract_atomic(label, bytes, target)
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
        let typst_path = bin_dir.join(binaries::TYPST_NAME);

        extract_one("Pandoc", binaries::PANDOC, &pandoc_path)?;
        extract_one("PDFium", binaries::PDFIUM, &pdfium_path)?;
        extract_one("typst", binaries::TYPST, &typst_path)?;

        Ok(ExtractedPaths {
            pandoc: pandoc_path,
            pdfium: pdfium_path,
            typst: typst_path,
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

pub fn get_typst_path() -> Result<&'static PathBuf, AppError> {
    Ok(&get_extracted_paths()?.typst)
}
