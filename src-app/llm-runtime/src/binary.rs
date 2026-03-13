//! Binary discovery for engine executables
//!
//! Discovers engine binaries downloaded from GitHub releases.
//! Searches in the following locations:
//! 1. Same directory as executable
//! 2. bin/ subdirectory
//! 3. System PATH
//! 4. macOS: ../Resources/bin/ (production bundles)

use std::path::{Path, PathBuf};

use crate::config::EngineType;
use crate::error::{Result, RuntimeError};

/// Get the path to an engine binary
pub fn get_engine_binary_path(engine: EngineType) -> Result<PathBuf> {
    // Discover binary using standard search paths
    let binary_name = match engine {
        EngineType::Llamacpp => "llama-server",
        EngineType::Mistralrs => "mistralrs-server",
    };

    find_executable_binary(binary_name).ok_or_else(|| {
        RuntimeError::BinaryNotFound(format!(
            "Engine binary '{}' not found. Searched in:\n\
             1. Same directory as executable\n\
             2. bin/ subdirectory\n\
             3. System PATH\n\
             4. macOS: ../Resources/bin/\n\
             \n\
             Tip: Download binaries from GitHub releases and place them in one of the above locations.",
            binary_name
        ))
    })
}

/// Find executable binary with fallback search (mirrors ResourcePaths::find_executable_binary)
fn find_executable_binary(binary_name: &str) -> Option<PathBuf> {
    // Get executable directory
    let exe_path = std::env::current_exe().ok()?;
    let exe_dir = exe_path.parent()?;

    // Platform-specific binary name
    let binary_filename = if cfg!(target_os = "windows") {
        format!("{}.exe", binary_name)
    } else {
        binary_name.to_string()
    };

    // 1. Try same directory as executable
    let primary_path = exe_dir.join(&binary_filename);
    if primary_path.exists() {
        return Some(primary_path);
    }

    // 2. macOS: Check production bundle first (Resources/bin)
    #[cfg(target_os = "macos")]
    {
        let resources_bin_path = exe_dir.join("../Resources/bin").join(&binary_filename);
        if resources_bin_path.exists() {
            return Some(resources_bin_path);
        }

        // Then check development location (bin)
        let dev_bin_path = exe_dir.join("bin").join(&binary_filename);
        if dev_bin_path.exists() {
            return Some(dev_bin_path);
        }
    }

    // 3. Other platforms: Try bin/ subdirectory
    #[cfg(not(target_os = "macos"))]
    {
        let bin_path = exe_dir.join("bin").join(&binary_filename);
        if bin_path.exists() {
            return Some(bin_path);
        }
    }

    // 4. Try PATH
    which::which(&binary_filename).ok()
}

/// Resolve binary path using a glob pattern
/// This is used in dev mode to find binaries in build directories
///
/// Example pattern: "target/release/build/*/out/*/bin/llama-server"
pub fn resolve_binary_with_glob(pattern: &str) -> Result<PathBuf> {
    use std::fs;

    // Expand glob pattern
    let paths: Vec<PathBuf> = glob::glob(pattern)
        .map_err(|e| {
            RuntimeError::BinaryNotFound(format!("Invalid glob pattern '{}': {}", pattern, e))
        })?
        .filter_map(|entry| entry.ok())
        .filter(|path| path.is_file())
        .collect();

    if paths.is_empty() {
        return Err(RuntimeError::BinaryNotFound(format!(
            "No binaries found matching pattern: {}",
            pattern
        )));
    }

    // Sort by modification time, newest first
    let mut paths_with_time: Vec<(PathBuf, std::time::SystemTime)> = paths
        .into_iter()
        .filter_map(|path| {
            fs::metadata(&path)
                .ok()
                .and_then(|m| m.modified().ok())
                .map(|time| (path, time))
        })
        .collect();

    paths_with_time.sort_by(|a, b| b.1.cmp(&a.1));

    paths_with_time
        .first()
        .map(|(path, _)| path.clone())
        .ok_or_else(|| {
            RuntimeError::BinaryNotFound(format!(
                "Could not determine newest binary from pattern: {}",
                pattern
            ))
        })
}

/// Ensure a binary is executable (Unix only)
#[cfg(unix)]
pub fn ensure_executable(path: &Path) -> Result<()> {
    use std::fs;
    use std::os::unix::fs::PermissionsExt;

    let metadata = fs::metadata(path)?;
    let mut permissions = metadata.permissions();

    // Set executable bit for owner
    permissions.set_mode(permissions.mode() | 0o100);
    fs::set_permissions(path, permissions)?;

    Ok(())
}

#[cfg(not(unix))]
pub fn ensure_executable(_path: &Path) -> Result<()> {
    // Windows doesn't need explicit executable permission
    Ok(())
}
