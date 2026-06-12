//! Build-time copy of the tracked hub seed into the include-dir!
//! source.
//!
//! Design: the seed is a tracked snapshot under
//! `resources/hub-seed/` (committed to git, evolved by maintainers
//! syncing from `ziee-ai/hub`'s Pages branch). This helper just
//! copies that snapshot to `binaries/hub-seed/` where
//! `hub_manager.rs` bakes it in with `include_dir!`.
//!
//! What this REPLACES (deleted with the Pages migration):
//!   - GitHub Releases tag resolution + 6-artifact download
//!   - sha256 sidecar verification
//!   - cosign keyless verification via sigstore
//!   - tarball download + bomb-guarded unpack
//!   - HUB_RELEASE_TAG pinning + GITHUB_TOKEN handling
//!   - per-build `.tag` cache + flock-based race protection
//!
//! Trust model is HTTPS-only (the Pages branch is the canonical
//! source); the runtime refresh path validates fetched catalog JSON
//! against the embedded JSON Schema rather than against a Sigstore
//! signature.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

/// Build-helper entry point — called from `build.rs`. Mirrors the
/// peer-helper signature `(target, target_dir, out_dir)` but only
/// uses `out_dir` (for the version sidecar). The seed is target-
/// agnostic — same JSON on every platform.
pub fn setup_hub_seed(
    _target: &str,
    _target_dir: &Path,
    out_dir: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let source_dir = manifest_dir.join("resources").join("hub-seed");
    let dest_dir = manifest_dir.join("binaries").join("hub-seed");

    // Re-run when any file inside the tracked seed changes. Cargo
    // watches directory mtimes only on *some* platforms when given
    // a dir path, so emitting per-file rerun lines is the durable
    // way to catch in-place edits to index.json / per-entry files.
    println!("cargo:rerun-if-changed={}", source_dir.display());
    emit_per_file_rerun(&source_dir);

    if !source_dir.exists() {
        return Err(format!(
            "hub-seed: missing tracked seed at {} — the seed is \
             tracked in-repo; restore it from git or pull from \
             ziee-ai/hub's Pages branch",
            source_dir.display()
        )
        .into());
    }

    // Atomic-ish replace: clear the dest then copy. The crate's
    // `include_dir!` re-evaluates at compile time from this dir,
    // so a half-written state is only visible to a build that
    // races us — which the workspace doesn't do for build helpers.
    if dest_dir.exists() {
        fs::remove_dir_all(&dest_dir)?;
    }
    fs::create_dir_all(&dest_dir)?;
    copy_dir_recursive(&source_dir, &dest_dir)?;

    // Resolve the seed version: read `resources/hub-seed/index.json`
    // and look for the catalog's `hub_version` field. The catalog
    // carries `hub_version` as a build-marker (separate from the
    // per-entry `version` envelope on each item).
    let version = read_seed_version(&dest_dir).unwrap_or_else(|err| {
        // Don't fail the build over a missing version — emit `0.0.0`
        // and warn so the binary still links. A bad seed will be
        // caught by the runtime test `seed_index_version_matches_const`.
        println!("cargo:warning=hub-seed: {} — defaulting to 0.0.0", err);
        "0.0.0".to_string()
    });
    write_version_file_atomically(out_dir, &version)?;

    Ok(())
}

/// Recursively copy `src` → `dst`. Pre-existing dst is the caller's
/// problem; this just walks and writes. Symlinks are NOT followed
/// (the seed is JSON-only — there should be no symlinks anyway).
fn copy_dir_recursive(src: &Path, dst: &Path) -> std::io::Result<()> {
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());
        if file_type.is_dir() {
            fs::create_dir_all(&dst_path)?;
            copy_dir_recursive(&src_path, &dst_path)?;
        } else if file_type.is_file() {
            fs::copy(&src_path, &dst_path)?;
        }
        // Symlinks are skipped — see above.
    }
    Ok(())
}

/// Emit `cargo:rerun-if-changed=<path>` for every file under `root`
/// recursively. Cargo's directory-mtime watch misses in-place edits;
/// per-file watches are the safe option.
fn emit_per_file_rerun(root: &Path) {
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            emit_per_file_rerun(&path);
        } else {
            println!("cargo:rerun-if-changed={}", path.display());
        }
    }
}

/// Read the catalog version from `<seed>/index.json`. Returns a
/// human error string on missing file / bad JSON / missing field.
fn read_seed_version(seed_dir: &Path) -> Result<String, String> {
    let index_path = seed_dir.join("index.json");
    let bytes = fs::read(&index_path).map_err(|e| {
        format!("read {} for hub_version: {}", index_path.display(), e)
    })?;
    let json: serde_json::Value = serde_json::from_slice(&bytes)
        .map_err(|e| format!("parse {} as JSON: {}", index_path.display(), e))?;
    json.get("hub_version")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| {
            format!("missing `hub_version` in {}", index_path.display())
        })
}

/// Write the resolved version to `$OUT_DIR/hub_seed_version.txt`
/// via tmp + rename so a SIGKILL mid-write leaves the previous
/// file untouched rather than truncating it to zero bytes.
fn write_version_file_atomically(
    out_dir: &str,
    version: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let final_path = Path::new(out_dir).join("hub_seed_version.txt");
    let tmp_path = final_path.with_extension("txt.tmp");
    {
        let mut f = fs::File::create(&tmp_path)?;
        f.write_all(version.as_bytes())?;
        f.write_all(b"\n")?;
        f.sync_all()?;
    }
    fs::rename(&tmp_path, &final_path)?;
    Ok(())
}
