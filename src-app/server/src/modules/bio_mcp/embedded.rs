//! Embedded BioMCP binary, extracted to `{app_data_dir}/bin/` on first
//! use. The binary is staged at build time by `build_helper/biomcp.rs`
//! and baked in via `include_bytes!`.
//!
//! Fail-soft: when the build helper could not fetch a real binary it
//! stages a ZERO-BYTE stub, so the embedded payload is empty.
//! [`biomcp_available`] reports that, and the module self-disables at
//! boot instead of trying to spawn a 0-byte "binary".
//!
//! The supported-triple set is identical to the MCP uv/bun embed
//! (`mcp/utils/embedded.rs`); other triples already fail the whole
//! server build there, so no extra handling is needed here. Keep the
//! `#[cfg(...)]` arms below in sync with the triple `match` in
//! `build_helper/biomcp.rs` (which stages the file these `include_bytes!`).

use once_cell::sync::OnceCell;
use std::fs;
use std::path::PathBuf;

use crate::common::AppError;

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
mod binaries {
    pub const BIOMCP: &[u8] = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/binaries/x86_64-unknown-linux-gnu/biomcp/biomcp"
    ));
    pub const BIOMCP_NAME: &str = "biomcp";
}

#[cfg(all(target_os = "linux", target_arch = "aarch64"))]
mod binaries {
    pub const BIOMCP: &[u8] = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/binaries/aarch64-unknown-linux-gnu/biomcp/biomcp"
    ));
    pub const BIOMCP_NAME: &str = "biomcp";
}

#[cfg(all(target_os = "macos", target_arch = "x86_64"))]
mod binaries {
    pub const BIOMCP: &[u8] = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/binaries/x86_64-apple-darwin/biomcp/biomcp"
    ));
    pub const BIOMCP_NAME: &str = "biomcp";
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
mod binaries {
    pub const BIOMCP: &[u8] = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/binaries/aarch64-apple-darwin/biomcp/biomcp"
    ));
    pub const BIOMCP_NAME: &str = "biomcp";
}

#[cfg(all(target_os = "windows", target_arch = "x86_64"))]
mod binaries {
    pub const BIOMCP: &[u8] = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/binaries/x86_64-pc-windows-msvc/biomcp/biomcp.exe"
    ));
    pub const BIOMCP_NAME: &str = "biomcp.exe";
}

// Unsupported platforms already fail to compile at mcp/utils/embedded.rs
// (uv/bun share this triple set); keep a matching guard for clarity.
#[cfg(not(any(
    all(target_os = "linux", target_arch = "x86_64"),
    all(target_os = "linux", target_arch = "aarch64"),
    all(target_os = "macos", target_arch = "x86_64"),
    all(target_os = "macos", target_arch = "aarch64"),
    all(target_os = "windows", target_arch = "x86_64")
)))]
compile_error!(
    "BioMCP embedded binary is not available for this platform. \
     Supported: Linux (x86_64, aarch64), macOS (x86_64, aarch64), Windows (x86_64)."
);

/// True when a real (non-stub) biomcp binary is embedded. False means the
/// build helper staged a zero-byte stub (no network / missing asset /
/// checksum mismatch) → the module self-disables.
pub fn biomcp_available() -> bool {
    !binaries::BIOMCP.is_empty()
}

static EXTRACTED_PATH: OnceCell<PathBuf> = OnceCell::new();

/// Extract the embedded biomcp binary to `{app_data_dir}/bin/` (once) and
/// return its path. Errors if no real binary is embedded.
pub fn ensure_biomcp_extracted() -> Result<&'static PathBuf, AppError> {
    EXTRACTED_PATH.get_or_try_init(|| {
        if !biomcp_available() {
            return Err(AppError::internal_error(
                "BioMCP binary unavailable (build staged a stub); feature disabled",
            ));
        }

        let app_data_dir = crate::core::get_app_data_dir();
        let bin_dir = app_data_dir.join("bin");
        fs::create_dir_all(&bin_dir).map_err(|e| {
            AppError::internal_error(format!("Failed to create bin directory: {}", e))
        })?;

        let biomcp_path = bin_dir.join(binaries::BIOMCP_NAME);
        if !biomcp_path.exists() {
            tracing::info!("Extracting embedded BioMCP binary to {:?}", biomcp_path);
            fs::write(&biomcp_path, binaries::BIOMCP).map_err(|e| {
                AppError::internal_error(format!("Failed to write BioMCP binary: {}", e))
            })?;

            #[cfg(unix)]
            set_executable(&biomcp_path)?;

            tracing::info!("BioMCP binary extracted ({} bytes)", binaries::BIOMCP.len());
        } else {
            tracing::debug!("BioMCP binary already extracted at {:?}", biomcp_path);
        }

        Ok(biomcp_path)
    })
}

#[cfg(unix)]
fn set_executable(path: &PathBuf) -> Result<(), AppError> {
    use std::os::unix::fs::PermissionsExt;
    let mut perms = fs::metadata(path)
        .map_err(|e| AppError::internal_error(format!("Failed to stat BioMCP binary: {}", e)))?
        .permissions();
    perms.set_mode(0o755);
    fs::set_permissions(path, perms).map_err(|e| {
        AppError::internal_error(format!("Failed to set BioMCP executable bit: {}", e))
    })?;
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;


    /// `biomcp_available()` is exactly "a non-stub binary is embedded".
    #[test]
    fn availability_reflects_embedded_payload() {
        assert_eq!(biomcp_available(), !binaries::BIOMCP.is_empty());
    }


    /// Extraction contract is consistent with availability: a stub build errors
    /// out (feature self-disables), a real build extracts a non-empty binary to
    /// disk. Exercises the real `ensure_biomcp_extracted` path either way.
    #[test]
    fn ensure_extracted_matches_availability() {
        if biomcp_available() {
            let path = ensure_biomcp_extracted().expect("a real binary must extract");
            assert!(path.exists(), "extracted binary must exist on disk");
            assert!(
                fs::metadata(path).unwrap().len() > 0,
                "extracted binary must be non-empty"
            );
        } else {
            assert!(
                ensure_biomcp_extracted().is_err(),
                "a stub build must report the binary unavailable"
            );
        }
    }


    /// On Unix the extracted binary MUST be marked executable (0o755) or the
    /// supervisor can't spawn it.
    #[cfg(unix)]
    #[test]
    fn set_executable_sets_0755() {
        use std::os::unix::fs::PermissionsExt;

        let dir = std::env::temp_dir().join(format!(
            "ziee-biomcp-exec-test-{}-{}",
            std::process::id(),
            line!()
        ));
        fs::create_dir_all(&dir).unwrap();
        let f = dir.join("fake-biomcp");
        fs::write(&f, b"#!/bin/sh\ntrue\n").unwrap();

        // Start NON-executable.
        let mut perms = fs::metadata(&f).unwrap().permissions();
        perms.set_mode(0o644);
        fs::set_permissions(&f, perms).unwrap();

        set_executable(&f).expect("set_executable");

        let mode = fs::metadata(&f).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o755, "binary must be chmod 0o755 after extraction");

        let _ = fs::remove_dir_all(&dir);
    }


    /// The embedded-binary extraction path (ensure_biomcp_extracted) is
    /// build-conditioned (a real binary vs a zero-byte stub), so assert its
    /// fail-soft CONTRACT either way: extraction succeeds IFF a real binary is
    /// embedded; on success the file is written to disk with exactly the embedded
    /// bytes. Drives the real fn (writes to the ambient app_data_dir; idempotent).
    #[test]
    fn ensure_biomcp_extracted_matches_availability() {
        let avail = biomcp_available();
        match ensure_biomcp_extracted() {
            Ok(p) => {
                assert!(avail, "extraction succeeded → a real binary must be embedded");
                assert!(p.exists(), "the extracted binary must exist on disk");
                let len = std::fs::metadata(p).unwrap().len() as usize;
                assert_eq!(len, binaries::BIOMCP.len(), "on-disk size matches the embedded bytes");
                assert!(len > 0, "a real embedded binary is non-empty");
            }
            Err(_) => {
                assert!(!avail, "extraction fails ONLY for a stub build (no real binary)");
            }
        }
    }
}
