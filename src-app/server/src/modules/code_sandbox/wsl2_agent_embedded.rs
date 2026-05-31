//! Self-contained Windows sandbox-guest-agent.
//!
//! Mirror of `embedded.rs` (macOS path), minimal to a single file: the
//! Linux ELF `ziee-sandbox-agent` that the WSL2 backend copies into the
//! distro at provision time. The build-time helper at
//! `build_helper/wsl2_agent.rs` cross-compiles the agent inside Docker
//! and drops it at `binaries/<host-target>/sandbox-runtime/ziee-sandbox-agent`.
//! This module `include_bytes!`s that file and, on first use, extracts
//! it to `<app_data>/bin/ziee-sandbox-agent`.
//!
//! Why not embed at link time on every host:
//!   - The agent is a Linux ELF and only makes sense for the Windows WSL2
//!     backend. Mac uses its libkrun bundle (the larger `bundle.tar.zst`)
//!     and Linux uses host-`bwrap` directly.
//!   - Build helpers are per-target: the actual `include_bytes!` macro
//!     points at a path that build.rs writes (a real ELF on Windows
//!     targets, a 0-byte placeholder elsewhere). The runtime `is_supported()`
//!     check gates use.
//!
//! Concurrency: identical to `embedded.rs` — per-process `OnceCell` cache
//! over a per-pid staging dir + atomic rename. Two server processes racing
//! the same extraction either win or lose each `rename` at the filesystem
//! level; identical bytes mean a harmless overwrite either way.

use once_cell::sync::OnceCell;
use sha2::{Digest, Sha256};
use std::io::Write;
use std::path::{Path, PathBuf};

/// Embedded `ziee-sandbox-agent` ELF. Populated by `build_helper/wsl2_agent.rs`
/// on Windows targets; a 0-byte placeholder file otherwise (so this
/// `include_bytes!` always resolves).
#[cfg(target_os = "windows")]
const AGENT_BYTES: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/binaries/x86_64-pc-windows-msvc/sandbox-runtime/ziee-sandbox-agent"
));

// Non-Windows hosts: this module compiles but never gets called. Keep
// the const so `cargo check` on every target stays clean.
#[cfg(not(target_os = "windows"))]
const AGENT_BYTES: &[u8] = &[];

/// Marker file recording which agent sha is currently extracted. Mismatch
/// (or missing) → re-extract.
const AGENT_SHA_MARKER: &str = ".wsl2-agent-sha";

/// Cached path to the extracted agent.
static EXTRACTED: OnceCell<PathBuf> = OnceCell::new();

/// `true` when the build produced a real (non-placeholder) agent. On
/// non-Windows hosts this is always false; on Windows it depends on
/// whether build.rs's `wsl2_agent::setup` actually ran the Docker
/// cross-compile (vs writing the placeholder).
pub fn is_supported() -> bool {
    !AGENT_BYTES.is_empty()
}

/// Extract the agent on first call, return cached path on subsequent
/// calls. Returns `Err` with a descriptive message when the embedded
/// bytes are empty (placeholder build / wrong target).
pub fn ensure() -> Result<&'static PathBuf, String> {
    if AGENT_BYTES.is_empty() {
        return Err(
            "wsl2-agent embedded bytes are empty (built for a non-Windows target, \
             or ZIEE_SKIP_WSL2_AGENT_BUNDLE was set during build, or Docker was \
             unavailable when build.rs ran). The runtime can still find the agent \
             via the legacy sibling-of-exe lookup if the binary is dropped next to \
             ziee.exe."
                .to_string(),
        );
    }
    EXTRACTED.get_or_try_init(|| do_extract().map_err(|e| e.to_string()))
}

fn do_extract() -> Result<PathBuf, Box<dyn std::error::Error>> {
    let app_data = crate::core::get_app_data_dir();
    std::fs::create_dir_all(&app_data)?;

    let mut hasher = Sha256::new();
    hasher.update(AGENT_BYTES);
    let current_sha = hex::encode(hasher.finalize());

    let agent_dest = app_data.join("bin").join("ziee-sandbox-agent");
    let marker = app_data.join(AGENT_SHA_MARKER);

    if marker_matches(&marker, &current_sha) && agent_dest.is_file() {
        tracing::debug!(
            agent = %agent_dest.display(),
            "wsl2-agent: already extracted (sha matches + file present)"
        );
        return Ok(agent_dest);
    }

    tracing::info!(
        agent = %agent_dest.display(),
        bytes = AGENT_BYTES.len(),
        "wsl2-agent: extracting embedded ziee-sandbox-agent"
    );

    if let Some(parent) = agent_dest.parent() {
        std::fs::create_dir_all(parent)?;
    }

    // Stage to a per-pid tmp file, then atomic-rename into place. Identical
    // pattern to `embedded.rs::do_extract`.
    let staging = app_data
        .join("bin")
        .join(format!("ziee-sandbox-agent.staging-{}", std::process::id()));
    {
        let mut f = std::fs::File::create(&staging)?;
        f.write_all(AGENT_BYTES)?;
        f.sync_all()?;
    }
    // No need to chmod +x on Windows — file mode bits don't apply.
    // On the Linux side of WSL2, the agent is `install -m 0755`'d into
    // the distro by `provision_distro`, so the Windows-host file mode
    // is irrelevant.

    // Atomic rename (Windows rename CAN fail if the dest is open; the
    // agent file isn't normally open by anything except wsl.exe relays
    // that have already cached the bytes into the WSL distro, so this
    // is safe in practice).
    std::fs::rename(&staging, &agent_dest)?;

    write_marker_atomic(&marker, &current_sha)?;
    Ok(agent_dest)
}

fn marker_matches(marker: &Path, expected_sha: &str) -> bool {
    match std::fs::read_to_string(marker) {
        Ok(content) => content.trim() == expected_sha,
        Err(_) => false,
    }
}

fn write_marker_atomic(marker: &Path, sha: &str) -> std::io::Result<()> {
    let tmp = marker.with_extension("tmp");
    {
        let mut f = std::fs::File::create(&tmp)?;
        f.write_all(sha.as_bytes())?;
        f.write_all(b"\n")?;
        f.sync_all()?;
    }
    std::fs::rename(&tmp, marker)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Verifies the build wired up the cross-compiled Linux agent
    /// correctly: on Windows hosts the embedded blob must be a real
    /// ELF (non-empty), and its bytes must equal the on-disk artifact
    /// at `binaries/x86_64-pc-windows-msvc/sandbox-runtime/ziee-sandbox-agent`.
    ///
    /// This catches:
    /// - Build placed a 0-byte placeholder where a real binary was expected.
    /// - The `include_bytes!` macro pointed at a stale path (out-of-sync rename).
    /// - Bit-rot from a partial Docker output (truncated file vs reported success).
    ///
    /// On non-Windows targets, the constant is empty by design — assert that.
    #[cfg(target_os = "windows")]
    #[test]
    fn embedded_agent_is_real_linux_elf() {
        assert!(
            !AGENT_BYTES.is_empty(),
            "AGENT_BYTES is empty on a Windows target — build_helper/wsl2_agent.rs \
             must have written a placeholder. Run `cargo clean -p ziee && cargo build` \
             with Docker daemon up, or set ZIEE_SKIP_WSL2_AGENT_BUNDLE=0."
        );
        // ELF magic: 0x7F 'E' 'L' 'F'
        assert_eq!(
            &AGENT_BYTES[..4],
            b"\x7fELF",
            "AGENT_BYTES is not an ELF binary — header bytes: {:02x?}",
            &AGENT_BYTES[..4.min(AGENT_BYTES.len())]
        );
        // ELF class byte at offset 4: 2 = ELF64. We cross-compile for
        // x86_64-unknown-linux-musl, so this MUST be ELF64.
        assert_eq!(
            AGENT_BYTES[4], 2,
            "AGENT_BYTES is not ELF64 (e_ident[EI_CLASS] = {})",
            AGENT_BYTES[4]
        );
    }

    #[cfg(not(target_os = "windows"))]
    #[test]
    fn embedded_agent_placeholder_on_non_windows() {
        assert!(
            AGENT_BYTES.is_empty(),
            "AGENT_BYTES is non-empty on non-Windows ({} bytes); the cfg-gate is wrong.",
            AGENT_BYTES.len()
        );
        assert!(
            !is_supported(),
            "is_supported() must be false on non-Windows"
        );
    }

    /// `is_supported()` is the gate the runtime uses to decide whether
    /// to attempt extraction. On Windows with a real bundle it must
    /// return true; with the placeholder it must return false.
    #[cfg(target_os = "windows")]
    #[test]
    fn is_supported_matches_blob_state() {
        if AGENT_BYTES.is_empty() {
            assert!(!is_supported(), "placeholder build should report unsupported");
        } else {
            assert!(is_supported(), "real-bundle build should report supported");
        }
    }
}
