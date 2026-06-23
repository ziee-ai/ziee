//! Build-time copy (+ pack) of the tracked hub seed into the
//! include-dir! source.
//!
//! Design: the seed is a tracked snapshot under
//! `resources/hub-seed/` (committed to git, evolved by maintainers
//! syncing from `ziee-ai/hub`'s Pages branch). This helper copies
//! that snapshot to `binaries/hub-seed/` where `hub_manager.rs`
//! bakes it in with `include_dir!`.
//!
//! Two bundle shapes are supported in the tracked seed:
//!   - **prebuilt** — a `{ver}.tar.gz` + manifest (the mirrored
//!     `ziee-ai/hub` Pages entries). Copied through verbatim.
//!   - **source-form** — the editable entry-point (`workflow.yaml`)
//!     committed loose instead of an opaque tarball. `pack_source_bundles`
//!     tars it at build time, writes the `.tar.gz`, and fills in the
//!     manifest's `sha256`/`size_bytes`/`file_count` in the BAKED copy
//!     (the committed manifest omits those build-computed fields). This
//!     keeps git diffs reviewable and makes bundle↔manifest drift
//!     impossible — the hash is always recomputed from the source.
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

    // Pack any SOURCE-FORM bundle into the `.tar.gz` its manifest references.
    // A source-form bundle is a dir whose manifest's `bundle.entry_point` file
    // is present loose (the editable `workflow.yaml` we commit to git instead of
    // an opaque tarball). We tar the loose source, write the archive, refresh the
    // manifest's sha256/size/file_count, and drop the loose files from the baked
    // dir so the result is a normal manifest+tarball bundle. Bundles that ship a
    // prebuilt tarball and NO loose source (the mirrored hub entries) are skipped.
    pack_source_bundles(&dest_dir)?;

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

/// Walk the seed and pack every source-form bundle (see the call site). For
/// each `*.json` manifest carrying a `bundle` block whose `entry_point` file is
/// present loose in the same dir, deterministically tar the loose source files
/// (everything except `*.json` manifests and `*.tar.gz` archives), write the
/// archive at the basename of `bundle.url`, rewrite the manifest's
/// `sha256`/`size_bytes`/`file_count`, then delete the now-packed loose files so
/// the baked dir matches the normal manifest+tarball bundle layout.
fn pack_source_bundles(seed_root: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let mut dirs = Vec::new();
    collect_dirs(seed_root, &mut dirs);
    for dir in dirs {
        for entry in fs::read_dir(&dir)? {
            let manifest_path = entry?.path();
            if manifest_path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            let Ok(raw) = fs::read(&manifest_path) else { continue };
            let Ok(mut manifest) = serde_json::from_slice::<serde_json::Value>(&raw) else {
                continue;
            };
            let bundle = match manifest.get("bundle") {
                Some(b) => b.clone(),
                None => continue,
            };
            let Some(entry_point) = bundle.get("entry_point").and_then(|v| v.as_str()) else {
                continue;
            };
            let Some(url) = bundle.get("url").and_then(|v| v.as_str()) else {
                continue;
            };
            // No loose entry-point source → a prebuilt (mirrored) bundle; leave it.
            if !dir.join(entry_point).exists() {
                continue;
            }
            let tar_name = Path::new(url)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("bundle.tar.gz")
                .to_string();

            let mut files = Vec::new();
            collect_bundle_files(&dir, &dir, &mut files);
            files.sort();
            let (tar_gz, file_count) = build_tar_gz(&dir, &files)?;

            fs::write(dir.join(&tar_name), &tar_gz)?;
            if let Some(b) = manifest.get_mut("bundle").and_then(|v| v.as_object_mut()) {
                b.insert("sha256".into(), serde_json::Value::String(hex_sha256(&tar_gz)));
                b.insert("size_bytes".into(), serde_json::json!(tar_gz.len() as u64));
                b.insert("file_count".into(), serde_json::json!(file_count));
            }
            let mut pretty = serde_json::to_vec_pretty(&manifest)?;
            pretty.push(b'\n');
            fs::write(&manifest_path, pretty)?;

            for rel in &files {
                let _ = fs::remove_file(dir.join(rel));
            }
            remove_empty_subdirs(&dir);
        }
    }
    Ok(())
}

/// Collect `root` and every directory beneath it (the seed is shallow).
fn collect_dirs(root: &Path, out: &mut Vec<PathBuf>) {
    out.push(root.to_path_buf());
    if let Ok(rd) = fs::read_dir(root) {
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                collect_dirs(&p, out);
            }
        }
    }
}

/// Relative paths (under `root`) of the files that belong INSIDE the bundle
/// tarball: every regular file except `*.json` manifests and `*.tar.gz`/`*.gz`
/// archives. Recurses so a `scripts/` subdir is packed too.
fn collect_bundle_files(root: &Path, dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            collect_bundle_files(root, &p, out);
        } else if p.is_file() {
            let ext = p.extension().and_then(|x| x.to_str()).unwrap_or("");
            let name = p.file_name().and_then(|x| x.to_str()).unwrap_or("");
            if ext == "json" || ext == "gz" || name.ends_with(".tar.gz") {
                continue;
            }
            if let Ok(rel) = p.strip_prefix(root) {
                out.push(rel.to_path_buf());
            }
        }
    }
}

/// Deterministically tar+gzip `rel_files` (relative to `root`). Fixed mtime/
/// uid/gid + sorted input + a fixed compression level make the bytes (and thus
/// the sha256) reproducible. Files under a top-level `scripts/` dir keep the
/// execute bit (sandbox steps run them); everything else is `0o644`.
fn build_tar_gz(
    root: &Path,
    rel_files: &[PathBuf],
) -> Result<(Vec<u8>, u32), Box<dyn std::error::Error>> {
    use flate2::Compression;
    use flate2::write::GzEncoder;
    use tar::{Builder, Header};
    let enc = GzEncoder::new(Vec::new(), Compression::new(6));
    let mut builder = Builder::new(enc);
    let mut count = 0u32;
    for rel in rel_files {
        let data = fs::read(root.join(rel))?;
        let exec = rel
            .components()
            .next()
            .map(|c| c.as_os_str() == "scripts")
            .unwrap_or(false);
        let mut header = Header::new_gnu();
        header.set_size(data.len() as u64);
        header.set_mode(if exec { 0o755 } else { 0o644 });
        header.set_mtime(0);
        header.set_uid(0);
        header.set_gid(0);
        header.set_entry_type(tar::EntryType::Regular);
        header.set_cksum();
        let rel_str = rel.to_string_lossy().replace('\\', "/");
        builder.append_data(&mut header, &rel_str, data.as_slice())?;
        count += 1;
    }
    let enc = builder.into_inner()?;
    let bytes = enc.finish()?;
    Ok((bytes, count))
}

/// Lowercase-hex sha256 of `bytes`.
fn hex_sha256(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(bytes);
    h.finalize().iter().map(|b| format!("{b:02x}")).collect()
}

/// Remove now-empty subdirectories (e.g. an emptied `scripts/`) after their
/// files were packed into the tarball. Best-effort; only removes empties.
fn remove_empty_subdirs(dir: &Path) {
    if let Ok(rd) = fs::read_dir(dir) {
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                remove_empty_subdirs(&p);
                let _ = fs::remove_dir(&p);
            }
        }
    }
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
