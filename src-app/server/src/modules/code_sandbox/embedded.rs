#![cfg_attr(not(target_os = "macos"), allow(dead_code))]
//! Self-contained mac-arm64 sandbox runtime.
//!
//! The build-time helper at `build_helper/sandbox_runtime.rs` assembles
//! the launcher binary + 5 brew dylibs + Alpine guest root + cross-built
//! guest agent into one tar.zst at
//! `binaries/aarch64-apple-darwin/sandbox-runtime/bundle.tar.zst`.
//! This module `include_bytes!`s that artifact and, on first sandbox
//! use, extracts it FLAT into `<app_data_dir>/{bin,lib,share,etc}/`
//! alongside the other extracted artifacts (pandoc, libpdfium, uv, bun)
//! that the file/utils and mcp/utils embedded modules write to `bin/`.
//!
//! A single marker file `<app_data_dir>/.sandbox-bundle-sha` holds the
//! sha256 of the currently-extracted bundle. On startup we compare it
//! to the embedded bundle's sha; mismatch (or missing) triggers a
//! selective wipe of only the items this bundle owns (launcher + 5
//! dylibs + share/guest-root/ + etc/entitlements.plist) followed by a
//! fresh extract. Sibling files in `bin/` (pandoc/pdfium/uv/bun) and
//! top-level dirs (`sandbox-rootfs/`, `postgres/`, `postgres-data/`,
//! `models/`, `hf-models/`, `llm-engines/`, `cache/`, `sandboxes/`,
//! `workspaces/`) are NEVER touched.
//!
//! Codesigning happens after extraction completes: libkrun refuses to
//! `krun_start_enter` without `com.apple.security.hypervisor` on the
//! launcher. The entitlements plist ships inside the bundle as
//! `etc/entitlements.plist`.
//!
//! Only the mac-arm64 build embeds a real bundle. Every other target
//! gets a 0-byte placeholder (the build helper writes one) so this
//! module compiles everywhere; callers must check `is_supported()`
//! before invoking `ensure()` on non-mac-arm64 targets.

use once_cell::sync::OnceCell;
use sha2::{Digest, Sha256};
use std::io::Write;
use std::path::Path;

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
const BUNDLE: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/binaries/aarch64-apple-darwin/sandbox-runtime/bundle.tar.zst"
));

#[cfg(not(all(target_os = "macos", target_arch = "aarch64")))]
const BUNDLE: &[u8] = &[];

/// Marker file (lives directly under `<app_data>`) recording which
/// bundle sha is currently extracted. A future entry in this module
/// could rotate to `.sandbox-bundle-sha.v2` if the marker format
/// itself ever changes.
const BUNDLE_SHA_MARKER: &str = ".sandbox-bundle-sha";

/// Items the embedded bundle owns. These are the ONLY paths under
/// `<app_data>/` that we wipe on a bundle-sha mismatch. Sibling files
/// in the same dirs (pandoc/pdfium/uv/bun in `bin/`, anything in
/// `sandbox-rootfs/`/`postgres/`/etc.) are explicitly NOT touched.
///
/// `(relative_path, kind)` — kind is "file" or "dir" so the wipe pass
/// can pick the right `remove_*` call. Adding a new file to a future
/// bundle means adding it here.
const OWNED_ITEMS: &[(&str, ItemKind)] = &[
    ("bin/ziee-sandbox-vm-launcher", ItemKind::File),
    ("lib/libkrun.1.dylib", ItemKind::File),
    ("lib/libkrunfw.5.dylib", ItemKind::File),
    ("lib/libepoxy.0.dylib", ItemKind::File),
    ("lib/libvirglrenderer.1.dylib", ItemKind::File),
    ("lib/libMoltenVK.dylib", ItemKind::File),
    ("share/guest-root", ItemKind::Dir),
    ("etc/entitlements.plist", ItemKind::File),
];

#[derive(Clone, Copy)]
enum ItemKind {
    File,
    Dir,
}

// `Extracted` moved to the sandbox engine (`ziee_sandbox::provider`) so the
// engine's `GuestAgentProvider` trait + `mac_vm` backend can name the return
// shape; this module (whose `include_bytes!` needs the SERVER
// `CARGO_MANIFEST_DIR`) keeps the extraction body and returns the engine type.
use ziee_sandbox::provider::Extracted;

static EXTRACTED: OnceCell<Extracted> = OnceCell::new();

/// True when the build produced a real (non-placeholder) bundle.
pub fn is_supported() -> bool {
    !BUNDLE.is_empty()
}

/// Extract the bundle on first call, return cached paths on subsequent
/// calls. Idempotent across processes via the per-file atomic-rename
/// swap in `do_extract`.
pub fn ensure() -> Result<&'static Extracted, String> {
    if BUNDLE.is_empty() {
        return Err(
            "sandbox-runtime bundle is empty (built for an unsupported target, or \
             ZIEE_SKIP_SANDBOX_BUNDLE was set during build)"
                .to_string(),
        );
    }
    EXTRACTED.get_or_try_init(|| do_extract().map_err(|e| e.to_string()))
}

fn do_extract() -> Result<Extracted, Box<dyn std::error::Error>> {
    let app_data = crate::core::get_app_data_dir();
    std::fs::create_dir_all(&app_data)?;

    let mut hasher = Sha256::new();
    hasher.update(BUNDLE);
    let current_sha = hex::encode(hasher.finalize());

    let launcher = app_data.join("bin").join("ziee-sandbox-vm-launcher");
    let guest_root = app_data.join("share").join("guest-root");
    let marker = app_data.join(BUNDLE_SHA_MARKER);

    if marker_matches(&marker, &current_sha) && all_owned_items_exist(&app_data) {
        tracing::debug!(
            app_data = %app_data.display(),
            "sandbox-runtime: bundle already extracted (sha matches + items present)"
        );
        return Ok(Extracted { launcher, guest_root });
    }

    tracing::info!(
        app_data = %app_data.display(),
        bytes = BUNDLE.len(),
        "sandbox-runtime: extracting embedded bundle (flat layout)"
    );

    selective_wipe(&app_data);

    // Extract into a per-pid staging dir under <app_data>/.sandbox-staging-<pid>/
    // so two concurrent server processes don't fight over the same staging
    // tree. Each will do its own extraction, then the per-file atomic rename
    // pass below races at the filesystem level (POSIX rename is atomic per
    // file; identical content wins either way).
    let staging = app_data.join(format!(".sandbox-staging-{}", std::process::id()));
    if staging.exists() {
        std::fs::remove_dir_all(&staging)?;
    }
    std::fs::create_dir_all(&staging)?;

    let unpack_result = (|| -> Result<(), Box<dyn std::error::Error>> {
        let decompressed = zstd::decode_all(std::io::Cursor::new(BUNDLE))?;
        let mut archive = tar::Archive::new(std::io::Cursor::new(decompressed));
        archive.set_preserve_permissions(true);
        archive.set_unpack_xattrs(false);
        archive.unpack(&staging)?;
        Ok(())
    })();
    if let Err(e) = unpack_result {
        let _ = std::fs::remove_dir_all(&staging);
        return Err(e);
    }

    // Codesign the staged launcher BEFORE moving it into place, so a
    // racing reader (e.g. another process extracting the same bundle)
    // never observes an unsigned `bin/ziee-sandbox-vm-launcher`.
    let staged_launcher = staging.join("bin").join("ziee-sandbox-vm-launcher");
    let staged_entitlements = staging.join("etc").join("entitlements.plist");
    if !staged_launcher.is_file() {
        let _ = std::fs::remove_dir_all(&staging);
        return Err(format!("launcher missing after unpack: {}", staged_launcher.display()).into());
    }
    if !staged_entitlements.is_file() {
        let _ = std::fs::remove_dir_all(&staging);
        return Err(format!(
            "entitlements.plist missing in bundle: {}",
            staged_entitlements.display()
        )
        .into());
    }
    let codesign = std::process::Command::new("/usr/bin/codesign")
        .args([
            "--force",
            "-s",
            "-",
            "--entitlements",
            &staged_entitlements.to_string_lossy(),
            "--timestamp=none",
            &staged_launcher.to_string_lossy(),
        ])
        .output()?;
    if !codesign.status.success() {
        let _ = std::fs::remove_dir_all(&staging);
        return Err(format!(
            "codesign failed (exit {:?}): {}",
            codesign.status.code(),
            String::from_utf8_lossy(&codesign.stderr)
        )
        .into());
    }

    // Per-file atomic rename swap. POSIX `rename` is atomic per-entry
    // and replaces the destination if it exists. Concurrent extractors
    // either win or lose each rename — identical content means harmless
    // overwrite. For the directory `share/guest-root`, rename moves the
    // entire subtree atomically.
    promote_staging_into_place(&staging, &app_data)?;

    // Marker last, AFTER all items are in place. Write to a tmp + rename
    // so a crash mid-write doesn't leave a half-truncated marker that
    // confuses the next boot's sha comparison.
    write_marker_atomic(&marker, &current_sha)?;

    // Best-effort: tear down our staging dir. Racing processes' staging
    // dirs each have a distinct pid suffix so we never step on theirs.
    let _ = std::fs::remove_dir_all(&staging);

    tracing::info!(
        launcher = %launcher.display(),
        guest_root = %guest_root.display(),
        "sandbox-runtime: extraction complete (flat layout)"
    );
    Ok(Extracted { launcher, guest_root })
}

fn marker_matches(marker: &Path, expected_sha: &str) -> bool {
    match std::fs::read_to_string(marker) {
        Ok(content) => content.trim() == expected_sha,
        Err(_) => false,
    }
}

fn all_owned_items_exist(app_data: &Path) -> bool {
    OWNED_ITEMS.iter().all(|(rel, kind)| {
        let p = app_data.join(rel);
        match kind {
            ItemKind::File => p.is_file(),
            ItemKind::Dir => p.is_dir(),
        }
    })
}

/// Remove every path in OWNED_ITEMS. Best-effort: a missing item is
/// fine (mid-upgrade state). A failure to remove is logged but
/// non-fatal — the subsequent atomic-rename pass will overwrite the
/// file regardless. NEVER touches anything outside OWNED_ITEMS.
fn selective_wipe(app_data: &Path) {
    for (rel, kind) in OWNED_ITEMS {
        let p = app_data.join(rel);
        let result = match kind {
            ItemKind::File => std::fs::remove_file(&p),
            ItemKind::Dir => std::fs::remove_dir_all(&p),
        };
        if let Err(e) = result {
            if e.kind() != std::io::ErrorKind::NotFound {
                tracing::warn!(
                    path = %p.display(),
                    "sandbox-runtime: selective-wipe could not remove {e}; proceeding"
                );
            }
        }
    }
}

/// For each file under `staging`, atomically rename it to the
/// corresponding path under `dest`, creating parent dirs as needed.
/// Directories are walked (not renamed wholesale) so a sibling
/// in `dest/bin/` like `pandoc` survives even when we touch
/// `dest/bin/ziee-sandbox-vm-launcher`.
fn promote_staging_into_place(staging: &Path, dest: &Path) -> std::io::Result<()> {
    promote_recursive(staging, staging, dest)
}

fn promote_recursive(root: &Path, current: &Path, dest_root: &Path) -> std::io::Result<()> {
    for entry in std::fs::read_dir(current)? {
        let entry = entry?;
        let path = entry.path();
        let rel = path
            .strip_prefix(root)
            .expect("entry must be under staging root");
        let dest_path = dest_root.join(rel);
        if path.is_dir() {
            std::fs::create_dir_all(&dest_path)?;
            promote_recursive(root, &path, dest_root)?;
        } else {
            if let Some(parent) = dest_path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            // POSIX rename: atomic per-file, replaces existing target.
            // If the dest is currently mmap'd by a running launcher,
            // the rename succeeds (mac kernel keeps the old inode for
            // open handles) and the new file lands; the running
            // process keeps its prior mapped pages until exit.
            std::fs::rename(&path, &dest_path)?;
        }
    }
    Ok(())
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

/// Decompressed size, exposed for the self-contained verification test.
#[cfg(test)]
pub(crate) fn bundle_bytes() -> &'static [u8] {
    BUNDLE
}
