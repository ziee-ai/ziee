// Embedded MCP binaries (UV and Bun) for all supported platforms
// Binaries are embedded at compile time and extracted to app_data_dir/bin/ at runtime

use once_cell::sync::OnceCell;
use std::fs;
use std::path::PathBuf;

use crate::common::AppError;

// =====================================================
// Platform-specific Binary Embedding
// =====================================================

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
mod binaries {
    pub const UV: &[u8] = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/binaries/x86_64-unknown-linux-gnu/uv/uv"
    ));
    pub const BUN: &[u8] = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/binaries/x86_64-unknown-linux-gnu/bun/bun"
    ));
    pub const UV_NAME: &str = "uv";
    pub const BUN_NAME: &str = "bun";
}

#[cfg(all(target_os = "linux", target_arch = "aarch64"))]
mod binaries {
    pub const UV: &[u8] = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/binaries/aarch64-unknown-linux-gnu/uv/uv"
    ));
    pub const BUN: &[u8] = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/binaries/aarch64-unknown-linux-gnu/bun/bun"
    ));
    pub const UV_NAME: &str = "uv";
    pub const BUN_NAME: &str = "bun";
}

#[cfg(all(target_os = "macos", target_arch = "x86_64"))]
mod binaries {
    pub const UV: &[u8] = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/binaries/x86_64-apple-darwin/uv/uv"
    ));
    pub const BUN: &[u8] = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/binaries/x86_64-apple-darwin/bun/bun"
    ));
    pub const UV_NAME: &str = "uv";
    pub const BUN_NAME: &str = "bun";
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
mod binaries {
    pub const UV: &[u8] = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/binaries/aarch64-apple-darwin/uv/uv"
    ));
    pub const BUN: &[u8] = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/binaries/aarch64-apple-darwin/bun/bun"
    ));
    pub const UV_NAME: &str = "uv";
    pub const BUN_NAME: &str = "bun";
}

#[cfg(all(target_os = "windows", target_arch = "x86_64"))]
mod binaries {
    pub const UV: &[u8] = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/binaries/x86_64-pc-windows-msvc/uv/uv.exe"
    ));
    pub const BUN: &[u8] = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/binaries/x86_64-pc-windows-msvc/bun/bun.exe"
    ));
    pub const UV_NAME: &str = "uv.exe";
    pub const BUN_NAME: &str = "bun.exe";
}

// Compile-time error for unsupported platforms
#[cfg(not(any(
    all(target_os = "linux", target_arch = "x86_64"),
    all(target_os = "linux", target_arch = "aarch64"),
    all(target_os = "macos", target_arch = "x86_64"),
    all(target_os = "macos", target_arch = "aarch64"),
    all(target_os = "windows", target_arch = "x86_64")
)))]
compile_error!(
    "MCP embedded binaries are not available for this platform. \
     Supported platforms: Linux (x86_64, aarch64), macOS (x86_64, aarch64), Windows (x86_64). \
     Please install uv and bun manually: https://github.com/astral-sh/uv and https://bun.sh/"
);

// =====================================================
// Runtime Extraction
// =====================================================

static EXTRACTED_PATHS: OnceCell<ExtractedPaths> = OnceCell::new();

struct ExtractedPaths {
    uv: PathBuf,
    bun: PathBuf,
}

/// Ensure embedded binaries are extracted to {app_data_dir}/bin/
/// This is called once during MCP module initialization
pub fn ensure_binaries_extracted() -> Result<(), AppError> {
    EXTRACTED_PATHS
        .get_or_try_init(|| {
            let app_data_dir = crate::core::get_app_data_dir();
            let bin_dir = app_data_dir.join("bin");

            // Create bin directory if it doesn't exist
            fs::create_dir_all(&bin_dir).map_err(|e| {
                AppError::internal_error(&format!("Failed to create bin directory: {}", e))
            })?;

            // Extract UV
            let uv_path = bin_dir.join(binaries::UV_NAME);
            if !uv_path.exists() {
                tracing::info!("Extracting embedded UV binary to {:?}", uv_path);
                fs::write(&uv_path, binaries::UV).map_err(|e| {
                    AppError::internal_error(&format!("Failed to write UV binary: {}", e))
                })?;

                #[cfg(unix)]
                set_executable(&uv_path)?;

                tracing::info!("UV binary extracted successfully");
            } else {
                tracing::debug!("UV binary already exists at {:?}", uv_path);
            }

            // Extract Bun
            let bun_path = bin_dir.join(binaries::BUN_NAME);
            if !bun_path.exists() {
                tracing::info!("Extracting embedded Bun binary to {:?}", bun_path);
                fs::write(&bun_path, binaries::BUN).map_err(|e| {
                    AppError::internal_error(&format!("Failed to write Bun binary: {}", e))
                })?;

                #[cfg(unix)]
                set_executable(&bun_path)?;

                tracing::info!("Bun binary extracted successfully");
            } else {
                tracing::debug!("Bun binary already exists at {:?}", bun_path);
            }

            Ok(ExtractedPaths { uv: uv_path, bun: bun_path })
        })
        .map(|_| ())
}

/// Get the path to the embedded UV binary
/// Returns an error if extraction hasn't been performed yet
pub fn get_uv_path() -> Result<&'static PathBuf, AppError> {
    EXTRACTED_PATHS
        .get()
        .map(|paths| &paths.uv)
        .ok_or_else(|| {
            AppError::internal_error("UV binary not extracted - ensure_binaries_extracted() must be called first")
        })
}

/// Get the path to the embedded Bun binary
/// Returns an error if extraction hasn't been performed yet
pub fn get_bun_path() -> Result<&'static PathBuf, AppError> {
    EXTRACTED_PATHS
        .get()
        .map(|paths| &paths.bun)
        .ok_or_else(|| {
            AppError::internal_error("Bun binary not extracted - ensure_binaries_extracted() must be called first")
        })
}

#[cfg(unix)]
fn set_executable(path: &PathBuf) -> Result<(), AppError> {
    use std::os::unix::fs::PermissionsExt;

    let mut perms = fs::metadata(path)
        .map_err(|e| AppError::internal_error(&format!("Failed to get file metadata: {}", e)))?
        .permissions();

    perms.set_mode(0o755);

    fs::set_permissions(path, perms)
        .map_err(|e| AppError::internal_error(&format!("Failed to set executable permissions: {}", e)))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_embedded_binaries_exist() {
        // Just verify the binaries are embedded (compile-time check)
        assert!(binaries::UV.len() > 0, "UV binary should be embedded");
        assert!(binaries::BUN.len() > 0, "Bun binary should be embedded");
    }
}
