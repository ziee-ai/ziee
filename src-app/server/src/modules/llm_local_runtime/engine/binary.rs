//! Executable-permission helper for downloaded engine binaries.
//!
//! The crate's PATH/glob binary-discovery helpers are dropped in the
//! server: the server resolves the engine binary from the registered
//! runtime-version's cached path (binary_manager), not by searching, so
//! only `ensure_executable` is needed here.

use std::path::Path;

use super::error::Result;

/// Ensure a binary is executable (Unix only).
#[cfg(unix)]
pub fn ensure_executable(path: &Path) -> Result<()> {
    use std::fs;
    use std::os::unix::fs::PermissionsExt;

    let metadata = fs::metadata(path)?;
    let mut permissions = metadata.permissions();
    // Set the executable bit for the owner.
    permissions.set_mode(permissions.mode() | 0o100);
    fs::set_permissions(path, permissions)?;
    Ok(())
}

#[cfg(not(unix))]
pub fn ensure_executable(_path: &Path) -> Result<()> {
    // Windows doesn't need an explicit executable permission.
    Ok(())
}
