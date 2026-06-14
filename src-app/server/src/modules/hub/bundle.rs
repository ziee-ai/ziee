//! Bundle download + safe extract for hub skills / workflows.
//!
//! Per plan §3 + §4.5 staging contract.
//!
//! Flow (`fetch_and_extract`):
//! 1. GET `<pages-base>/<bundle.url>` — streamed with a 10 MiB hard
//!    cap (mirrors `hub_manager::MAX_HUB_ARTIFACT_BYTES` semantics for
//!    bundle artifacts).
//! 2. Stream the body into a temp file under
//!    `<target_dir>/.staging/<uuid>/bundle.tar.gz`.
//! 3. Verify sha256 matches `bundle.sha256` exactly — reject on
//!    mismatch.
//! 4. Extract via `tar` + `flate2` with the §1 bomb guards:
//!    - cumulative decompressed bytes > 10 MiB → abort
//!    - file count > 256 → abort
//!    - single file > 2 MiB → abort
//!    - symlinks → reject
//!    - non-regular entries (devices, FIFOs, hardlinks) → reject
//!    - paths containing `..` or absolute → reject
//!    - drop execute bits for SKILLS (skill scripts deferred); preserve
//!      for WORKFLOWS (sandbox steps need them)
//! 5. Atomic rename `.staging/<uuid>/extracted/` → `target_dir`.
//!
//! `extract_from_seed_bytes` runs the same pipeline against raw bytes
//! already in memory — used by the `include_dir!`-baked seed corpus
//! that ships with the binary (no network in air-gapped boot).

use std::fs;
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};
use std::time::Duration;

use flate2::read::GzDecoder;
use sha2::{Digest, Sha256};
use tar::Archive;
use uuid::Uuid;

use crate::common::AppError;
use crate::modules::hub::hub_manager::HubManager;
use crate::modules::hub::models::HubBundle;

// ============================================================
// Bundle limits (mirrors plan §1)
// ============================================================

/// Hard cap on cumulative decompressed bytes per bundle. Same value
/// as `hub_manager::MAX_HUB_ARTIFACT_BYTES`'s effective cap for small
/// JSON artifacts — bundle's are different artifacts but the spec
/// pins them at 10 MiB.
pub const MAX_BUNDLE_DECOMPRESSED_BYTES: u64 = 10 * 1024 * 1024;
/// Hard cap on the on-the-wire compressed bundle size. Same number
/// because the bundles compress, but compressed cap == decompressed
/// cap is the safe upper bound.
pub const MAX_BUNDLE_COMPRESSED_BYTES: u64 = 10 * 1024 * 1024;
pub const MAX_BUNDLE_FILE_COUNT: u32 = 256;
pub const MAX_BUNDLE_SINGLE_FILE_BYTES: u64 = 2 * 1024 * 1024;

/// Per-bundle HTTP timeout (matches the catalog-fetcher's
/// `HTTP_TIMEOUT` for parity).
const HTTP_TIMEOUT: Duration = Duration::from_secs(30);

/// Outcome of a successful extract.
#[derive(Debug, Clone)]
pub struct BundleExtraction {
    /// Final on-disk path of the extracted bundle.
    pub extracted_path: PathBuf,
    /// Number of regular files written.
    pub file_count: u32,
    /// Cumulative decompressed bytes written.
    pub total_bytes: u64,
    /// Verified sha256 of the bundle bytes (lowercase hex, 64 chars).
    /// On `fetch_and_extract` this equals the manifest's `bundle.sha256`;
    /// on `extract_from_seed_bytes` it's computed over the input
    /// `bytes`.
    pub sha256_hex: String,
}

/// Bundle classification — drives the "drop execute bits" / "preserve
/// execute bits" choice during extract.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BundleKind {
    /// Skill bundles: SKILL.md + reference files. Phase 1 strips
    /// execute bits (skill scripts deferred to Phase 2).
    Skill,
    /// Workflow bundles: workflow.yaml + scripts/. Sandbox steps need
    /// execute bits preserved so `python3 scripts/foo.py` works inside
    /// the bwrap rootfs.
    Workflow,
}

// ============================================================
// Public API
// ============================================================

/// Download + sha256 + bomb-guarded extract. The bundle URL is
/// joined against the hub's pages base via the supplied `hub_manager`
/// (which already owns the resolved base + seed fallback semantics).
pub async fn fetch_and_extract(
    hub_manager: &HubManager,
    bundle: &HubBundle,
    target_dir: &Path,
    kind: BundleKind,
) -> Result<BundleExtraction, AppError> {
    // The HubManager's base URL is private — we re-derive from the
    // same fn here. The relative bundle url is validated below.
    let base = pages_base_for(hub_manager);
    let rel = bundle.url.trim_start_matches('/');
    if rel.is_empty() {
        return Err(AppError::bad_request(
            "BUNDLE_URL_EMPTY",
            "hub bundle has empty url",
        ));
    }
    // Defense-in-depth path-safety on the url before HTTP join — the
    // catalog path validator already rejects `..` / absolute / etc.
    // for manifest paths; bundles ride alongside in the same trees.
    if !is_safe_bundle_rel(rel) {
        return Err(AppError::internal_error(format!(
            "hub: bundle url '{rel}' has unsafe characters"
        )));
    }
    let url = format!("{}/{}", base.trim_end_matches('/'), rel);

    // Sanity-cap before the GET fires. The publisher's size pre-check
    // bounds this; consumer re-verifies after download.
    if bundle.size_bytes > MAX_BUNDLE_COMPRESSED_BYTES {
        return Err(AppError::unprocessable_entity(
            "BUNDLE_TOO_LARGE",
            format!(
                "hub bundle declares {} bytes (cap {})",
                bundle.size_bytes, MAX_BUNDLE_COMPRESSED_BYTES
            ),
        ));
    }
    if bundle.file_count > MAX_BUNDLE_FILE_COUNT {
        return Err(AppError::unprocessable_entity(
            "BUNDLE_TOO_MANY_FILES",
            format!(
                "hub bundle declares {} files (cap {})",
                bundle.file_count, MAX_BUNDLE_FILE_COUNT
            ),
        ));
    }

    // Stage dir lives next to the final target so the atomic
    // `fs::rename` from staging->target stays on the same filesystem.
    // Using `<parent>/.staging/<uuid>/` keeps the per-install staging
    // out of any concurrent reader's path AND survives the case where
    // `target_dir` doesn't exist yet (fresh install).
    let staging_parent = target_dir
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| std::env::temp_dir());
    fs::create_dir_all(&staging_parent).map_err(|e| {
        AppError::internal_error(format!(
            "bundle: create staging parent {}: {}",
            staging_parent.display(),
            e
        ))
    })?;
    let staging_root = staging_parent.join(".staging").join(Uuid::new_v4().to_string());
    fs::create_dir_all(&staging_root).map_err(|e| {
        AppError::internal_error(format!(
            "bundle: create staging dir {}: {}",
            staging_root.display(),
            e
        ))
    })?;

    let download_path = staging_root.join("bundle.tar.gz");
    let url_for_blocking = url.clone();
    let download_path_for_blocking = download_path.clone();
    let expected_sha = bundle.sha256.to_lowercase();
    let download_result =
        tokio::task::spawn_blocking(move || {
            download_to_file(&url_for_blocking, &download_path_for_blocking)
        })
        .await
        .map_err(|e| {
            AppError::internal_error(format!("bundle: download join: {e}"))
        })?;
    let sha_actual = match download_result {
        Ok(s) => s,
        Err(e) => {
            let _ = fs::remove_dir_all(&staging_root);
            return Err(e);
        }
    };
    if sha_actual != expected_sha {
        let _ = fs::remove_dir_all(&staging_root);
        return Err(AppError::unprocessable_entity(
            "BUNDLE_SHA256_MISMATCH",
            format!(
                "hub bundle sha256 mismatch (expected {}, got {})",
                expected_sha, sha_actual
            ),
        ));
    }

    // Extract.
    let extracted_dir = staging_root.join("extracted");
    fs::create_dir_all(&extracted_dir).map_err(|e| {
        AppError::internal_error(format!(
            "bundle: create extracted dir {}: {}",
            extracted_dir.display(),
            e
        ))
    })?;
    let extract_result = {
        let bytes = match fs::read(&download_path) {
            Ok(b) => b,
            Err(e) => {
                let _ = fs::remove_dir_all(&staging_root);
                return Err(AppError::internal_error(format!(
                    "bundle: read staged tar.gz: {e}"
                )));
            }
        };
        extract_tar_gz_to(&bytes, &extracted_dir, kind)
    };
    let extraction = match extract_result {
        Ok(e) => e,
        Err(e) => {
            let _ = fs::remove_dir_all(&staging_root);
            return Err(e);
        }
    };

    // Atomic promote. If the target_dir already exists (re-install on
    // top of an existing extracted dir), nuke it first — same-name
    // (name, version) means the install handler upstream has already
    // decided to overwrite.
    if let Some(parent) = target_dir.parent() {
        fs::create_dir_all(parent).map_err(|e| {
            AppError::internal_error(format!(
                "bundle: create target parent {}: {}",
                parent.display(),
                e
            ))
        })?;
    }
    if target_dir.exists() {
        fs::remove_dir_all(target_dir).map_err(|e| {
            AppError::internal_error(format!(
                "bundle: remove prior target {}: {}",
                target_dir.display(),
                e
            ))
        })?;
    }
    fs::rename(&extracted_dir, target_dir).map_err(|e| {
        AppError::internal_error(format!(
            "bundle: promote {} -> {}: {}",
            extracted_dir.display(),
            target_dir.display(),
            e
        ))
    })?;
    let _ = fs::remove_dir_all(&staging_root);

    Ok(BundleExtraction {
        extracted_path: target_dir.to_path_buf(),
        file_count: extraction.file_count,
        total_bytes: extraction.total_bytes,
        sha256_hex: sha_actual,
    })
}

/// Extract a bundle that ships embedded in the binary (no network).
/// Bytes are the tar.gz contents (e.g. read from `include_dir!`).
/// Runs the same bomb-guard + path-safety pipeline and verifies the
/// sha256 against `bundle.sha256`.
pub async fn extract_from_seed_bytes(
    bundle: &HubBundle,
    bytes: &[u8],
    target_dir: &Path,
    kind: BundleKind,
) -> Result<BundleExtraction, AppError> {
    let sha_actual = hex_sha256(bytes);
    let expected = bundle.sha256.to_lowercase();
    if sha_actual != expected {
        return Err(AppError::unprocessable_entity(
            "BUNDLE_SHA256_MISMATCH",
            format!(
                "seed bundle sha256 mismatch (expected {}, got {})",
                expected, sha_actual
            ),
        ));
    }

    let staging_parent = target_dir
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| std::env::temp_dir());
    fs::create_dir_all(&staging_parent).map_err(|e| {
        AppError::internal_error(format!(
            "bundle: create seed staging parent {}: {}",
            staging_parent.display(),
            e
        ))
    })?;
    let staging_root = staging_parent.join(".staging").join(Uuid::new_v4().to_string());
    fs::create_dir_all(&staging_root).map_err(|e| {
        AppError::internal_error(format!(
            "bundle: create seed staging dir {}: {}",
            staging_root.display(),
            e
        ))
    })?;
    let extracted_dir = staging_root.join("extracted");
    fs::create_dir_all(&extracted_dir).map_err(|e| {
        AppError::internal_error(format!(
            "bundle: create seed extracted dir {}: {}",
            extracted_dir.display(),
            e
        ))
    })?;
    let extraction = match extract_tar_gz_to(bytes, &extracted_dir, kind) {
        Ok(e) => e,
        Err(e) => {
            let _ = fs::remove_dir_all(&staging_root);
            return Err(e);
        }
    };

    if let Some(parent) = target_dir.parent() {
        fs::create_dir_all(parent).map_err(|e| {
            AppError::internal_error(format!(
                "bundle: create seed target parent {}: {}",
                parent.display(),
                e
            ))
        })?;
    }
    if target_dir.exists() {
        fs::remove_dir_all(target_dir).map_err(|e| {
            AppError::internal_error(format!(
                "bundle: remove prior seed target {}: {}",
                target_dir.display(),
                e
            ))
        })?;
    }
    fs::rename(&extracted_dir, target_dir).map_err(|e| {
        AppError::internal_error(format!(
            "bundle: promote {} -> {}: {}",
            extracted_dir.display(),
            target_dir.display(),
            e
        ))
    })?;
    let _ = fs::remove_dir_all(&staging_root);

    Ok(BundleExtraction {
        extracted_path: target_dir.to_path_buf(),
        file_count: extraction.file_count,
        total_bytes: extraction.total_bytes,
        sha256_hex: sha_actual,
    })
}

// ============================================================
// Internals
// ============================================================

struct LocalExtraction {
    file_count: u32,
    total_bytes: u64,
}

/// Stream-extract a tar.gz with bomb guards + path safety. Caller
/// owns staging / promotion / cleanup.
fn extract_tar_gz_to(
    bytes: &[u8],
    target_dir: &Path,
    kind: BundleKind,
) -> Result<LocalExtraction, AppError> {
    let gz = GzDecoder::new(bytes);
    let mut archive = Archive::new(gz);
    archive.set_preserve_permissions(true);
    archive.set_overwrite(false);
    archive.set_unpack_xattrs(false);

    let mut total_bytes: u64 = 0;
    let mut file_count: u32 = 0;

    for entry_result in archive.entries().map_err(|e| {
        AppError::internal_error(format!("bundle: tar entries: {e}"))
    })? {
        let mut entry = entry_result.map_err(|e| {
            AppError::internal_error(format!("bundle: tar entry: {e}"))
        })?;
        let entry_type = entry.header().entry_type();

        // Reject anything but regular files + directories.
        if !entry_type.is_file() && !entry_type.is_dir() {
            return Err(AppError::unprocessable_entity(
                "BUNDLE_NON_REGULAR_ENTRY",
                format!(
                    "bundle entry kind {:?} not permitted (symlinks / devices / FIFOs / hardlinks rejected)",
                    entry_type
                ),
            ));
        }

        let path = entry
            .path()
            .map_err(|e| {
                AppError::internal_error(format!("bundle: entry path: {e}"))
            })?
            .into_owned();

        // Path safety: no `..`, no absolute, no Windows root.
        // Only allow Normal components.
        if path.is_absolute() {
            return Err(AppError::unprocessable_entity(
                "BUNDLE_ABSOLUTE_PATH",
                format!("bundle entry path {:?} is absolute", path),
            ));
        }
        for c in path.components() {
            match c {
                Component::Normal(_) => {}
                Component::CurDir => {} // tolerate `./`
                _ => {
                    return Err(AppError::unprocessable_entity(
                        "BUNDLE_UNSAFE_PATH",
                        format!("bundle entry path {:?} contains '..' / root / prefix component", path),
                    ));
                }
            }
        }

        if entry_type.is_dir() {
            let dest = target_dir.join(&path);
            fs::create_dir_all(&dest).map_err(|e| {
                AppError::internal_error(format!(
                    "bundle: mkdir {}: {}",
                    dest.display(),
                    e
                ))
            })?;
            continue;
        }

        // Regular file.
        let size = entry.header().size().map_err(|e| {
            AppError::internal_error(format!("bundle: header size: {e}"))
        })?;
        if size > MAX_BUNDLE_SINGLE_FILE_BYTES {
            return Err(AppError::unprocessable_entity(
                "BUNDLE_FILE_TOO_LARGE",
                format!(
                    "bundle entry {:?} is {} bytes (cap {})",
                    path, size, MAX_BUNDLE_SINGLE_FILE_BYTES
                ),
            ));
        }
        if total_bytes.saturating_add(size) > MAX_BUNDLE_DECOMPRESSED_BYTES {
            return Err(AppError::unprocessable_entity(
                "BUNDLE_DECOMPRESSED_TOO_LARGE",
                format!(
                    "bundle decompressed exceeds {} bytes",
                    MAX_BUNDLE_DECOMPRESSED_BYTES
                ),
            ));
        }
        if file_count >= MAX_BUNDLE_FILE_COUNT {
            return Err(AppError::unprocessable_entity(
                "BUNDLE_TOO_MANY_FILES",
                format!("bundle exceeds {} files", MAX_BUNDLE_FILE_COUNT),
            ));
        }

        let dest = target_dir.join(&path);
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent).map_err(|e| {
                AppError::internal_error(format!(
                    "bundle: mkdir {}: {}",
                    parent.display(),
                    e
                ))
            })?;
        }

        // Read the body into a Vec so we can re-cap on what the
        // entry actually carries (header size can lie about an
        // EOF-trimmed entry).
        let mut buf: Vec<u8> = Vec::with_capacity(size as usize);
        let mut reader = (&mut entry).take(MAX_BUNDLE_SINGLE_FILE_BYTES + 1);
        reader.read_to_end(&mut buf).map_err(|e| {
            AppError::internal_error(format!("bundle: read entry body: {e}"))
        })?;
        if buf.len() as u64 > MAX_BUNDLE_SINGLE_FILE_BYTES {
            return Err(AppError::unprocessable_entity(
                "BUNDLE_FILE_TOO_LARGE",
                format!(
                    "bundle entry {:?} streamed > {} bytes",
                    path, MAX_BUNDLE_SINGLE_FILE_BYTES
                ),
            ));
        }
        if total_bytes.saturating_add(buf.len() as u64) > MAX_BUNDLE_DECOMPRESSED_BYTES {
            return Err(AppError::unprocessable_entity(
                "BUNDLE_DECOMPRESSED_TOO_LARGE",
                format!(
                    "bundle decompressed exceeds {} bytes",
                    MAX_BUNDLE_DECOMPRESSED_BYTES
                ),
            ));
        }
        total_bytes += buf.len() as u64;
        file_count += 1;

        let mut f = fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&dest)
            .map_err(|e| {
                AppError::internal_error(format!(
                    "bundle: open dest {}: {}",
                    dest.display(),
                    e
                ))
            })?;
        f.write_all(&buf).map_err(|e| {
            AppError::internal_error(format!(
                "bundle: write {}: {}",
                dest.display(),
                e
            ))
        })?;
        drop(f);

        // Per-kind permission policy.
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = entry.header().mode().unwrap_or(0o644);
            let new_mode = match kind {
                // Skills: strip execute bits (skill scripts are
                // Phase 2). 0o644 for regular files.
                BundleKind::Skill => mode & 0o666,
                // Workflows: preserve execute bits (sandbox steps
                // need `chmod +x scripts/foo.py`); mask to 0o755 to
                // strip world-write.
                BundleKind::Workflow => mode & 0o755,
            };
            let _ = fs::set_permissions(
                &dest,
                fs::Permissions::from_mode(new_mode),
            );
        }
        // On Windows the execute bit doesn't exist; the kind
        // distinction is a Unix-only thing.
        #[cfg(not(unix))]
        {
            let _ = kind; // silence unused warning
        }
    }

    Ok(LocalExtraction {
        file_count,
        total_bytes,
    })
}

/// GET `url`, stream into `dest` with a hard cap, return the lowercase
/// hex sha256 of the bytes written. Caller owns `dest` and is
/// responsible for cleanup on error.
fn download_to_file(url: &str, dest: &Path) -> Result<String, AppError> {
    let client = reqwest::blocking::Client::builder()
        .timeout(HTTP_TIMEOUT)
        .user_agent(concat!("ziee/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|e| {
            AppError::internal_error(format!("bundle: http client: {e}"))
        })?;
    let resp = client
        .get(url)
        .send()
        .map_err(|e| {
            AppError::internal_error(format!("bundle: GET {url}: {e}"))
        })?
        .error_for_status()
        .map_err(|e| {
            AppError::internal_error(format!("bundle: GET {url}: {e}"))
        })?;
    if let Some(len) = resp.content_length()
        && len > MAX_BUNDLE_COMPRESSED_BYTES
    {
        return Err(AppError::unprocessable_entity(
            "BUNDLE_TOO_LARGE",
            format!(
                "bundle: {url} declares {len} bytes (cap {MAX_BUNDLE_COMPRESSED_BYTES})"
            ),
        ));
    }

    let mut file = fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(dest)
        .map_err(|e| {
            AppError::internal_error(format!(
                "bundle: open dest {}: {}",
                dest.display(),
                e
            ))
        })?;
    let mut hasher = Sha256::new();
    let mut total: u64 = 0;
    let mut reader = resp.take(MAX_BUNDLE_COMPRESSED_BYTES + 1);
    let mut buf = [0u8; 64 * 1024];
    loop {
        let n = reader
            .read(&mut buf)
            .map_err(|e| AppError::internal_error(format!("bundle: read {url}: {e}")))?;
        if n == 0 {
            break;
        }
        total = total.saturating_add(n as u64);
        if total > MAX_BUNDLE_COMPRESSED_BYTES {
            return Err(AppError::unprocessable_entity(
                "BUNDLE_TOO_LARGE",
                format!("bundle: {url} exceeded {MAX_BUNDLE_COMPRESSED_BYTES} bytes"),
            ));
        }
        hasher.update(&buf[..n]);
        file.write_all(&buf[..n]).map_err(|e| {
            AppError::internal_error(format!(
                "bundle: write {}: {}",
                dest.display(),
                e
            ))
        })?;
    }
    file.flush().map_err(|e| {
        AppError::internal_error(format!(
            "bundle: flush {}: {}",
            dest.display(),
            e
        ))
    })?;
    Ok(format!("{:x}", hasher.finalize()))
}

fn hex_sha256(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

/// Pages base mirrors what `hub_manager` resolves. Kept private here
/// since `HubManager` doesn't currently expose its base URL (no need
/// in the catalog flow). If the catalog ever needs to expose it,
/// switch this to a getter on `HubManager`.
fn pages_base_for(_hub_manager: &HubManager) -> String {
    // Debug-only override (mirrors `hub_manager::hub_pages_base`) so
    // integration tests can point at a mock Pages server. Release
    // builds always hit the real Pages base.
    if cfg!(debug_assertions)
        && let Ok(v) = std::env::var("ZIEE_HUB_PAGES_BASE")
        && !v.is_empty()
    {
        return v;
    }
    crate::modules::hub::hub_manager::DEFAULT_PAGES_BASE.to_string()
}

/// Defensive check on the relative bundle URL stored in the manifest
/// `bundle.url` field. Must look like `<category>/<ns>/<leaf>/<v>.tar.gz`
/// and contain no `..` / absolute / weird path components.
fn is_safe_bundle_rel(rel: &str) -> bool {
    if rel.is_empty() || rel.len() > 512 {
        return false;
    }
    if !rel.ends_with(".tar.gz") {
        return false;
    }
    let path = Path::new(rel);
    if path.is_absolute() {
        return false;
    }
    for c in path.components() {
        match c {
            Component::Normal(_) => {}
            _ => return false,
        }
    }
    // Must start with a known bundle folder.
    rel.starts_with("skills/") || rel.starts_with("workflows/")
}

// ============================================================
// Unit tests — bomb-guard coverage on the extractor.
// ============================================================
#[cfg(test)]
mod tests {
    use super::*;
    use flate2::Compression;
    use flate2::write::GzEncoder;
    use std::io::Cursor;
    use tar::{Builder, Header};
    use tempfile::tempdir;

    /// Build a tar.gz body with the given (path, mode, contents) tuples.
    /// Each entry is a regular file. Optionally appends an extra entry
    /// at the end via the closure (used for symlink injection).
    fn build_tar_gz(
        entries: &[(&str, u32, &[u8])],
        extras: Option<&dyn Fn(&mut Builder<GzEncoder<Cursor<Vec<u8>>>>)>,
    ) -> Vec<u8> {
        let buf: Vec<u8> = Vec::new();
        let cur = Cursor::new(buf);
        let enc = GzEncoder::new(cur, Compression::default());
        let mut builder = Builder::new(enc);
        for (path, mode, contents) in entries {
            let mut header = Header::new_gnu();
            header.set_size(contents.len() as u64);
            header.set_mode(*mode);
            header.set_entry_type(tar::EntryType::Regular);
            header.set_cksum();
            builder
                .append_data(&mut header, path, &contents[..])
                .expect("append_data");
        }
        if let Some(extra) = extras {
            extra(&mut builder);
        }
        let enc = builder.into_inner().expect("into_inner");
        let cur = enc.finish().expect("gz finish");
        cur.into_inner()
    }

    fn synth_bundle(sha256_hex: &str, size_bytes: u64, file_count: u32) -> HubBundle {
        HubBundle {
            url: "skills/io.example/test/1.0.0.tar.gz".to_string(),
            sha256: sha256_hex.to_string(),
            size_bytes,
            file_count,
            entry_point: "SKILL.md".to_string(),
        }
    }

    #[tokio::test]
    async fn extracts_minimal_skill_bundle() {
        let body = build_tar_gz(
            &[
                ("SKILL.md", 0o644, b"---\nname: x\ndescription: y\n---\nbody"),
                ("references/foo.md", 0o644, b"foo"),
            ],
            None,
        );
        let sha = hex_sha256(&body);
        let bundle = synth_bundle(&sha, body.len() as u64, 2);
        let tmp = tempdir().unwrap();
        let target = tmp.path().join("extracted");
        let res =
            extract_from_seed_bytes(&bundle, &body, &target, BundleKind::Skill)
                .await
                .expect("extract");
        assert_eq!(res.file_count, 2);
        assert!(target.join("SKILL.md").exists());
        assert!(target.join("references/foo.md").exists());
    }

    #[tokio::test]
    async fn rejects_sha256_mismatch() {
        let body = build_tar_gz(&[("SKILL.md", 0o644, b"body")], None);
        let mut bundle = synth_bundle("0".repeat(64).as_str(), body.len() as u64, 1);
        bundle.sha256 = "0".repeat(64);
        let tmp = tempdir().unwrap();
        let res = extract_from_seed_bytes(
            &bundle,
            &body,
            &tmp.path().join("e"),
            BundleKind::Skill,
        )
        .await;
        assert!(res.unwrap_err().to_string().contains("sha256"));
    }

    #[tokio::test]
    async fn rejects_path_traversal() {
        // tar::Builder rejects `..` at append time (good — it's the
        // first layer of defense). To exercise OUR extractor's path
        // safety check we bypass `append_data` and inject a malicious
        // entry via the raw header's `path_bytes` API, which writes
        // whatever bytes we hand it directly into the archive.
        let buf: Vec<u8> = Vec::new();
        let cur = Cursor::new(buf);
        let enc = GzEncoder::new(cur, Compression::default());
        let mut builder = Builder::new(enc);

        let mut header = Header::new_gnu();
        header.set_size(5);
        header.set_mode(0o644);
        header.set_entry_type(tar::EntryType::Regular);
        // Bypass the builder's `..` rejection by writing the path
        // directly into the GNU header (set_path_bytes is the escape
        // hatch the tar lib explicitly leaves open).
        header
            .as_gnu_mut()
            .unwrap()
            .name[..16]
            .copy_from_slice(b"../../etc/passwd");
        header.set_cksum();
        builder
            .append(&header, &b"pwned"[..])
            .expect("append raw header");

        let enc = builder.into_inner().expect("into_inner");
        let body = enc.finish().expect("gz finish").into_inner();

        let sha = hex_sha256(&body);
        let bundle = synth_bundle(&sha, body.len() as u64, 1);
        let tmp = tempdir().unwrap();
        let err = extract_from_seed_bytes(
            &bundle,
            &body,
            &tmp.path().join("e"),
            BundleKind::Skill,
        )
        .await
        .unwrap_err();
        // Could be rejected as `..` or as relative-from-unsafe;
        // either way OUR extractor refuses the entry.
        let msg = err.to_string().to_lowercase();
        assert!(
            msg.contains("path") || msg.contains("unsafe"),
            "expected path rejection, got: {err}"
        );
    }

    #[tokio::test]
    async fn rejects_symlink_entry() {
        let body = build_tar_gz(
            &[("legit.md", 0o644, b"x")],
            Some(&|b| {
                let mut header = Header::new_gnu();
                header.set_entry_type(tar::EntryType::Symlink);
                header.set_size(0);
                header
                    .set_link_name("/etc/passwd")
                    .expect("set_link_name");
                header.set_cksum();
                b.append_data(&mut header, "evil_link", std::io::empty())
                    .expect("symlink append");
            }),
        );
        let sha = hex_sha256(&body);
        let bundle = synth_bundle(&sha, body.len() as u64, 2);
        let tmp = tempdir().unwrap();
        let err = extract_from_seed_bytes(
            &bundle,
            &body,
            &tmp.path().join("e"),
            BundleKind::Skill,
        )
        .await
        .unwrap_err();
        assert!(err.to_string().contains("non-regular") || err.to_string().contains("symlink"));
    }

    #[tokio::test]
    async fn rejects_oversize_single_file() {
        let big = vec![b'a'; (MAX_BUNDLE_SINGLE_FILE_BYTES + 1) as usize];
        let body = build_tar_gz(&[("SKILL.md", 0o644, &big)], None);
        let sha = hex_sha256(&body);
        let bundle = synth_bundle(&sha, body.len() as u64, 1);
        let tmp = tempdir().unwrap();
        let err = extract_from_seed_bytes(
            &bundle,
            &body,
            &tmp.path().join("e"),
            BundleKind::Skill,
        )
        .await
        .unwrap_err();
        let msg = err.to_string().to_lowercase();
        // Bomb-guard wins — either the header-size check, the
        // streamed-read cap, or the cumulative-decompressed cap fires.
        assert!(
            msg.contains("decompressed")
                || msg.contains("bytes")
                || msg.contains("file"),
            "expected size rejection, got: {err}"
        );
    }

    #[test]
    fn safe_bundle_rel_validation() {
        assert!(is_safe_bundle_rel("skills/io.foo/bar/1.0.0.tar.gz"));
        assert!(is_safe_bundle_rel("workflows/io.foo/bar/2.3.4.tar.gz"));
        assert!(!is_safe_bundle_rel(""));
        assert!(!is_safe_bundle_rel("/skills/foo/1.0.tar.gz"));
        assert!(!is_safe_bundle_rel("skills/../etc/passwd"));
        assert!(!is_safe_bundle_rel("evil/x/1.0.tar.gz"));
        assert!(!is_safe_bundle_rel("skills/foo/1.0.json"));
    }
}
