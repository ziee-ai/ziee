//! Build-time fetch of the embedded `ziee-ai/hub` catalog seed.
//!
//! Why: `hub_manager.rs` runs `include_dir!("$CARGO_MANIFEST_DIR/binaries/hub-seed")`
//! at compile time to bake the seed catalog into the binary. The seed
//! USED to be a manually-curated snapshot at `resources/hub-seed/`,
//! which drifted whenever someone forgot to refresh + bump
//! `SEED_HUB_VERSION`. This helper replaces that with an automatic
//! build-time fetch: every `cargo build` pulls the latest non-
//! prerelease tag from `github.com/ziee-ai/hub`, verifies it the same
//! way the runtime refresh path does (sha256 + cosign keyless), and
//! stages it into `binaries/hub-seed/` for the include macro.
//!
//! Verification chain MIRRORS runtime
//! (`src/modules/hub/hub_manager.rs::verify_sha256_sidecar` +
//! `verify_cosign_bundle` + `unpack_safely`). Drift between this
//! helper and runtime is a security concern — when the runtime
//! verify code changes, update this file too.
//!
//! Failure mode: any error here `panic!`s out of build.rs and fails
//! the whole build. Per the design call: an offline / rate-limited
//! / GitHub-down build SHOULD fail loudly rather than ship a stale
//! or empty seed. Operators wanting reproducible / offline builds
//! pin a specific tag via the `HUB_RELEASE_TAG` env var (and stage
//! a matching `binaries/hub-seed/` dir if running fully offline).

use serde::Deserialize;
use std::fs;
use std::io::Read;
use std::path::Path;

// Match runtime constants exactly. If these change in the runtime,
// change here too — staying in lockstep is the point.
const HUB_REPO_OWNER: &str = "ziee-ai";
const HUB_REPO_NAME: &str = "hub";
const COSIGN_OIDC_ISSUER: &str = "https://token.actions.githubusercontent.com";

/// Same upper-bound the runtime uses
/// (`MAX_HUB_ARTIFACT_BYTES` in `hub_manager.rs`).
const MAX_HUB_ARTIFACT_BYTES: u64 = 32 * 1024 * 1024;

/// Same decompression-bomb guards as runtime `unpack_safely`.
const MAX_UNPACKED_BYTES: u64 = 256 * 1024 * 1024;
const MAX_ENTRIES: usize = 100_000;

#[derive(Debug, Clone, Deserialize)]
struct GhRelease {
    tag_name: String,
    #[serde(default)]
    prerelease: bool,
    #[serde(default)]
    draft: bool,
}

/// Entry point — called from build.rs. Writes the seed catalog into
/// `binaries/hub-seed/` and the resolved tag (with leading `v`
/// stripped, matching the existing `SEED_HUB_VERSION` const format)
/// into `$OUT_DIR/hub_seed_version.txt`.
pub fn setup_hub_seed(
    binaries_dir: &Path,
    out_dir: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    // Trigger a rebuild when either the env override or the cached
    // tag changes — cargo skips `setup_hub_seed` re-runs otherwise.
    println!("cargo:rerun-if-env-changed=HUB_RELEASE_TAG");

    // The helper is called with `binaries_dir = <manifest>/binaries/<target>`
    // (the per-target binary dir). We want a target-AGNOSTIC location
    // because the catalog content is identical on every platform —
    // step UP to the manifest's `binaries/` and into `hub-seed/`.
    let seed_dir = binaries_dir
        .parent()
        .expect("binaries_dir always has a parent (manifest/binaries)")
        .join("hub-seed");

    let tag_file = seed_dir.join(".tag");
    println!("cargo:rerun-if-changed={}", tag_file.display());

    // Resolve the desired tag. `HUB_RELEASE_TAG` env wins (reproducible
    // / offline builds); otherwise hit the GitHub API.
    let desired_tag = match std::env::var("HUB_RELEASE_TAG") {
        Ok(t) if !t.is_empty() => {
            println!("Using pinned HUB_RELEASE_TAG={}", t);
            t
        }
        _ => {
            let latest = resolve_latest_release()?;
            println!("Resolved latest hub release: {}", latest);
            latest
        }
    };

    // Skip-if-fresh: cached tag matches the desired one + the seed
    // directory looks intact (index.json present). A partial / failed
    // prior run would leave only some files; re-run from scratch in
    // that case.
    if seed_dir.join("index.json").exists()
        && let Ok(cached_tag) = fs::read_to_string(&tag_file)
        && cached_tag.trim() == desired_tag
    {
        println!("hub-seed cache hit for {} — skipping download", desired_tag);
        write_version_file(out_dir, &desired_tag)?;
        return Ok(());
    }

    // Cache miss → fresh download into OUT_DIR staging (so a failure
    // mid-way doesn't poison the durable cache).
    let staging = Path::new(out_dir).join("hub-seed-staging");
    if staging.exists() {
        fs::remove_dir_all(&staging)?;
    }
    fs::create_dir_all(&staging)?;

    let asset_base = format!(
        "https://github.com/{}/{}/releases/download/{}",
        HUB_REPO_OWNER, HUB_REPO_NAME, desired_tag,
    );

    let artifacts = [
        "hub.tar.gz",
        "hub.tar.gz.sha256",
        "hub.tar.gz.cosign.bundle",
        "hub.index.json",
        "hub.index.json.sha256",
        "hub.index.json.cosign.bundle",
    ];
    for name in artifacts {
        let url = format!("{}/{}", asset_base, name);
        let dest = staging.join(name);
        println!("Downloading {}", url);
        download_to_file(&url, &dest)?;
    }

    // sha256 verify (matches runtime `verify_sha256_sidecar`).
    verify_sha256_sidecar(
        &staging.join("hub.tar.gz"),
        &staging.join("hub.tar.gz.sha256"),
    )?;
    verify_sha256_sidecar(
        &staging.join("hub.index.json"),
        &staging.join("hub.index.json.sha256"),
    )?;

    // Cosign keyless verify against the release.yml identity for this
    // tag (matches runtime `verify_cosign_bundle`).
    let identity = cosign_expected_identity(&desired_tag);
    verify_cosign_bundle(
        &staging.join("hub.tar.gz.cosign.bundle"),
        &staging.join("hub.tar.gz"),
        &identity,
        COSIGN_OIDC_ISSUER,
    )
    .map_err(|e| format!("cosign verify hub.tar.gz: {}", e))?;
    verify_cosign_bundle(
        &staging.join("hub.index.json.cosign.bundle"),
        &staging.join("hub.index.json"),
        &identity,
        COSIGN_OIDC_ISSUER,
    )
    .map_err(|e| format!("cosign verify hub.index.json: {}", e))?;

    // Unpack the verified tarball into a separate staging-contents dir
    // so we can atomically swap into binaries/hub-seed without leaving
    // a half-extracted tree behind on failure.
    let unpacked = staging.join("contents");
    fs::create_dir_all(&unpacked)?;
    unpack_safely(&staging.join("hub.tar.gz"), &unpacked)?;

    // Runtime guarantees the served index.json matches the SIGNED
    // payload (the tarball's bundled index could theoretically differ
    // from the separately-signed index — overwriting here closes that
    // gap; see hub_manager.rs around line 798).
    fs::copy(
        staging.join("hub.index.json"),
        unpacked.join("index.json"),
    )?;
    // Some release builds also ship a duplicate `hub.index.json`
    // inside the tarball alongside the canonical `index.json`. Strip
    // it — we only need `index.json` (which the copy above already
    // points at the verified bundle); the duplicate just bloats the
    // include_dir! output by ~8 KB.
    let dup = unpacked.join("hub.index.json");
    if dup.exists() {
        let _ = fs::remove_file(&dup);
    }

    // Atomic rotate into the durable cache. Rename is per-platform but
    // works fine when source + dest are on the same fs (always true
    // for $OUT_DIR + $CARGO_MANIFEST_DIR/binaries/ on a normal build).
    if seed_dir.exists() {
        fs::remove_dir_all(&seed_dir)?;
    }
    fs::create_dir_all(seed_dir.parent().unwrap())?;
    fs::rename(&unpacked, &seed_dir)?;

    // Stamp the tag so the next build can skip-if-fresh.
    fs::write(&tag_file, &desired_tag)?;
    write_version_file(out_dir, &desired_tag)?;

    // Best-effort cleanup of staging — leftover files are harmless
    // (OUT_DIR gets nuked on cargo clean anyway).
    let _ = fs::remove_dir_all(&staging);

    println!("hub-seed staged at {} (tag {})", seed_dir.display(), desired_tag);
    Ok(())
}

/// Emit the resolved tag (sans leading `v`) into a file the main crate
/// `include_str!`s for `SEED_HUB_VERSION`. The runtime const is bare
/// semver (e.g. "0.0.1-alpha"), not the tag form ("v0.0.1-alpha").
fn write_version_file(out_dir: &str, tag: &str) -> Result<(), Box<dyn std::error::Error>> {
    let version = tag.strip_prefix('v').unwrap_or(tag);
    fs::write(
        Path::new(out_dir).join("hub_seed_version.txt"),
        version,
    )?;
    Ok(())
}

/// GitHub API: `GET /repos/<owner>/<repo>/releases?per_page=50` → pick
/// the newest non-prerelease tag (falling back to the newest
/// prerelease if no stable exists). Mirrors runtime
/// `resolve_latest_release` at hub_manager.rs:882-896.
fn resolve_latest_release() -> Result<String, Box<dyn std::error::Error>> {
    let url = format!(
        "https://api.github.com/repos/{}/{}/releases?per_page=50",
        HUB_REPO_OWNER, HUB_REPO_NAME,
    );
    // ureq 3.x doesn't expose a typed `.read_json::<T>()` on Body —
    // read to string + parse with serde_json. The releases list for
    // ziee-ai/hub is tiny (a few KB even at 50 entries), so the buffer
    // cost is negligible.
    let body_text = ureq::get(&url)
        .header("Accept", "application/vnd.github+json")
        .header("User-Agent", concat!("ziee-build/", env!("CARGO_PKG_VERSION")))
        .call()
        .map_err(|e| format!("list releases ({}): {}", url, e))?
        .into_body()
        .read_to_string()
        .map_err(|e| format!("read releases body: {}", e))?;
    let releases: Vec<GhRelease> =
        serde_json::from_str(&body_text).map_err(|e| format!("parse releases: {}", e))?;
    let installable: Vec<GhRelease> = releases.into_iter().filter(|r| !r.draft).collect();
    if let Some(stable) = installable.iter().find(|r| !r.prerelease) {
        return Ok(stable.tag_name.clone());
    }
    installable
        .into_iter()
        .next()
        .map(|r| r.tag_name)
        .ok_or_else(|| "no releases found on GitHub".into())
}

fn cosign_expected_identity(tag: &str) -> String {
    format!(
        "https://github.com/{}/{}/.github/workflows/release.yml@refs/tags/{}",
        HUB_REPO_OWNER, HUB_REPO_NAME, tag,
    )
}

/// Download `url` to `dest`. Same retry shape + size-cap as runtime
/// `download_to_file` at hub_manager.rs:898-968 (3 attempts, 2s
/// sleep, server-error retry), but uses ureq (already a build-dep)
/// instead of reqwest::blocking (which would pull more deps in at
/// build time). The shape of failures is the same.
fn download_to_file(url: &str, dest: &Path) -> Result<u64, Box<dyn std::error::Error>> {
    let mut last_err = String::new();
    for attempt in 1..=3u32 {
        match ureq::get(url)
            .header("User-Agent", concat!("ziee-build/", env!("CARGO_PKG_VERSION")))
            .call()
        {
            Ok(resp) => {
                if let Some(len_str) = resp.headers().get("content-length")
                    && let Ok(len_str) = len_str.to_str()
                    && let Ok(len) = len_str.parse::<u64>()
                    && len > MAX_HUB_ARTIFACT_BYTES
                {
                    return Err(format!(
                        "{}: declares {} bytes (cap {})",
                        url, len, MAX_HUB_ARTIFACT_BYTES
                    )
                    .into());
                }
                let mut file = fs::File::create(dest)?;
                let mut reader = resp.into_body().into_reader();
                match std::io::copy(&mut reader, &mut file) {
                    Ok(n) => return Ok(n),
                    Err(e) => {
                        last_err = format!("stream-to-file: {}", e);
                        let _ = fs::remove_file(dest);
                        if attempt < 3 {
                            std::thread::sleep(std::time::Duration::from_secs(2));
                            continue;
                        }
                        return Err(format!("download {}: {}", url, last_err).into());
                    }
                }
            }
            Err(e) => {
                last_err = format!("send: {}", e);
                if attempt < 3 {
                    std::thread::sleep(std::time::Duration::from_secs(2));
                    continue;
                }
                return Err(format!("download {}: {}", url, last_err).into());
            }
        }
    }
    Err(format!("download {}: {}", url, last_err).into())
}

/// sha256sum sidecar shape: `<hex>  <filename>\n`. Mirrors runtime
/// hub_manager.rs::verify_sha256_sidecar at line 986.
fn verify_sha256_sidecar(blob: &Path, sidecar: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let sidecar_text = fs::read_to_string(sidecar)
        .map_err(|e| format!("read sidecar {}: {}", sidecar.display(), e))?;
    let expected_hex = sidecar_text
        .split_whitespace()
        .next()
        .ok_or_else(|| format!("empty sha256 sidecar {}", sidecar.display()))?
        .to_lowercase();
    if expected_hex.len() != 64 || !expected_hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(format!("malformed sha256 in sidecar {}", sidecar.display()).into());
    }
    let actual_hex = sha256_file(blob)?;
    if actual_hex != expected_hex {
        return Err(format!(
            "sha256 mismatch for {}: expected {} got {}",
            blob.display(),
            expected_hex,
            actual_hex
        )
        .into());
    }
    Ok(())
}

fn sha256_file(path: &Path) -> Result<String, Box<dyn std::error::Error>> {
    use sha2::{Digest, Sha256};
    let mut f = fs::File::open(path)?;
    let mut h = Sha256::new();
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Ok(format!("{:x}", h.finalize()))
}

/// Cosign keyless verify via the `sigstore` blocking API. Mirrors
/// runtime hub_manager.rs::verify_cosign_bundle at line 1028.
/// Production trust root + the identity passed by the caller.
///
/// IMPORTANT: build.rs runs inside a `#[tokio::main]` runtime (because
/// build_helper/* needs sqlx for the SQLx compile-time validation
/// elsewhere). The sigstore "blocking" API internally calls
/// `tokio::runtime::Builder::new_current_thread().build().block_on(...)`
/// to drive its async Rekor/Fulcio fetches, and `block_on` from
/// within an active runtime panics with "Cannot start a runtime
/// from within a runtime". The workaround: run the verify on a
/// fresh OS thread that has no tokio runtime context.
fn verify_cosign_bundle(
    bundle_path: &Path,
    blob_path: &Path,
    identity: &str,
    issuer: &str,
) -> Result<(), String> {
    let bundle_path = bundle_path.to_path_buf();
    let blob_path = blob_path.to_path_buf();
    let identity = identity.to_string();
    let issuer = issuer.to_string();

    std::thread::spawn(move || -> Result<(), String> {
        use sigstore::bundle::Bundle;
        use sigstore::bundle::verify::blocking::Verifier;
        use sigstore::bundle::verify::policy::Identity;

        let bundle_json = fs::read_to_string(&bundle_path)
            .map_err(|e| format!("read bundle: {}", e))?;
        let bundle: Bundle =
            serde_json::from_str(&bundle_json).map_err(|e| format!("parse bundle: {}", e))?;
        let blob = fs::File::open(&blob_path).map_err(|e| format!("open blob: {}", e))?;
        let verifier = Verifier::production().map_err(|e| format!("trust root init: {}", e))?;
        let policy = Identity::new(&identity, &issuer);
        verifier
            .verify(blob, bundle, &policy, false)
            .map_err(|e| format!("signature verification: {}", e))?;
        Ok(())
    })
    .join()
    .map_err(|_| "cosign verify thread panicked".to_string())?
}

/// Tar extraction with traversal + decompression-bomb guards. Mirrors
/// runtime hub_manager.rs::unpack_safely at line 1053.
fn unpack_safely(tar_gz: &Path, dest: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let f = fs::File::open(tar_gz)?;
    let gz = flate2::read::GzDecoder::new(f);
    let mut archive = tar::Archive::new(gz);
    let mut total_unpacked: u64 = 0;
    let mut entry_count: usize = 0;

    for entry in archive.entries()? {
        let mut entry = entry?;
        entry_count += 1;
        if entry_count > MAX_ENTRIES {
            return Err("archive exceeds entry-count cap".into());
        }
        total_unpacked = total_unpacked.saturating_add(entry.header().size().unwrap_or(0));
        if total_unpacked > MAX_UNPACKED_BYTES {
            return Err("archive exceeds uncompressed-size cap".into());
        }

        let kind = entry.header().entry_type();
        if !(kind.is_file() || kind.is_dir()) {
            return Err(format!("non-regular archive entry: {:?}", kind).into());
        }

        let path = entry.path()?.into_owned();
        if path.is_absolute() {
            return Err(format!("absolute path in archive: {}", path.display()).into());
        }
        for component in path.components() {
            if matches!(component, std::path::Component::ParentDir) {
                return Err(format!("parent-dir component in archive: {}", path.display()).into());
            }
        }
        entry
            .unpack_in(dest)
            .map_err(|e| format!("unpack {}: {}", path.display(), e))?;
    }
    Ok(())
}

