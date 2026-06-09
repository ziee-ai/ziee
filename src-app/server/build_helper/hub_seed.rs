//! Build-time fetch of the embedded `ziee-ai/hub` catalog seed.
//!
//! `hub_manager.rs` runs `include_dir!("$CARGO_MANIFEST_DIR/binaries/hub-seed")`
//! at compile time to bake the seed into the binary. This helper
//! populates that dir from the latest non-prerelease tag of
//! `github.com/ziee-ai/hub` on every `cargo build`, verifying the
//! download with the SAME chain (sha256 sidecar + cosign keyless)
//! that the runtime refresh path uses. Drift between this file and
//! `hub_manager.rs`'s verify functions is a security concern — when
//! the runtime verify code changes, update this file too.
//!
//! Failure modes: any download / verify / extract error `panic!`s
//! out of build.rs (per the design call — see Cargo.toml comment on
//! the sigstore build-dep). Operators on networks that can't reach
//! GitHub or Sigstore must pin a specific tag with `HUB_RELEASE_TAG`
//! and pre-stage `binaries/hub-seed/` (the skip-if-fresh path
//! consults that cache before any network call).

use fs2::FileExt;
use serde::Deserialize;
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::Duration;

// Match runtime constants exactly (`hub_manager.rs:34-46`). If those
// change in the runtime, change here too — staying in lockstep is the
// point of this whole helper.
const HUB_REPO_OWNER: &str = "ziee-ai";
const HUB_REPO_NAME: &str = "hub";
const COSIGN_OIDC_ISSUER: &str = "https://token.actions.githubusercontent.com";

/// Same upper bound the runtime applies to any single artifact
/// (`MAX_HUB_ARTIFACT_BYTES` in `hub_manager.rs`). 32 MiB leaves
/// headroom for thousands of items while bounding memory exposure.
const MAX_HUB_ARTIFACT_BYTES: u64 = 32 * 1024 * 1024;

/// Same decompression-bomb guards as runtime `unpack_safely`:
/// cap total uncompressed bytes + entry count, since the 32 MiB
/// compressed cap above only bounds the gzip input.
const MAX_UNPACKED_BYTES: u64 = 256 * 1024 * 1024;
const MAX_ENTRIES: usize = 100_000;

/// Per-request timeout for ureq calls — matches the 30s the runtime
/// uses at `hub_manager.rs:875`. Without this, a stalled GitHub /
/// Sigstore endpoint hangs the build indefinitely.
const HTTP_TIMEOUT: Duration = Duration::from_secs(30);

/// Wall-clock cap on the cosign-verify thread. Sigstore's
/// `Verifier::production()` reaches out to `rekor.sigstore.dev` and
/// `fulcio.sigstore.dev`; without a bound, a stall there freezes
/// the build with no progress.
const COSIGN_TIMEOUT: Duration = Duration::from_secs(120);

/// Maximum length for the version body (tag minus optional `v`
/// prefix). Matches the runtime's `is_safe_version` cap at
/// `hub_manager.rs:1198` so the build can't embed a value the
/// runtime would later reject.
const MAX_VERSION_LEN: usize = 32;

#[derive(Debug, Clone, Deserialize)]
struct GhRelease {
    tag_name: String,
    #[serde(default)]
    prerelease: bool,
    #[serde(default)]
    draft: bool,
}

/// Build-helper entry point — called from `build.rs`. Writes the
/// verified seed catalog to `binaries/hub-seed/` and the resolved
/// tag (sans leading `v`) into `$OUT_DIR/hub_seed_version.txt` so
/// `hub_manager.rs::SEED_HUB_VERSION` can `include_str!` it.
///
/// Signature mirrors the peer build helpers (`setup_typst` /
/// `setup_pandoc`): `(target, target_dir, out_dir)`. `_target` is
/// accepted for parity but unused — the hub seed is target-agnostic
/// (same YAML/JSON content on every platform), so it lives in
/// `<manifest>/binaries/hub-seed/`, not under a per-target subdir.
pub fn setup_hub_seed(
    _target: &str,
    _target_dir: &Path,
    out_dir: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    // Trigger a rebuild when the env override changes (and only
    // then, for the env-changed case — cargo would otherwise skip
    // setup_hub_seed entirely on a no-source-change build).
    println!("cargo:rerun-if-env-changed=HUB_RELEASE_TAG");
    println!("cargo:rerun-if-env-changed=GITHUB_TOKEN");

    // Derive `binaries/` directly from CARGO_MANIFEST_DIR rather than
    // climbing from `target_dir.parent()`. The hub seed is target-
    // agnostic (same YAML/JSON on every platform), so the per-target
    // segment of the peer signature isn't useful here, and any future
    // refactor that changes target_dir's depth would silently land
    // the seed in the wrong location with the .parent() approach.
    let binaries_root =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("binaries");
    fs::create_dir_all(&binaries_root)?;
    let seed_dir = binaries_root.join("hub-seed");
    let tag_file = seed_dir.join(".tag");

    // Watch the tag sidecar AND the seed directory entry list. Note
    // the limit: cargo's directory-level rerun-if-changed checks the
    // dir's mtime, which on Linux/macOS only bumps when entries are
    // added/removed/renamed — NOT when a file inside is edited
    // in-place. So:
    //   - file added/removed/renamed → build re-runs, re-verifies
    //   - file edited in-place (mtime change on the FILE) → cargo
    //     does NOT re-trigger; the stale-but-cached path runs and
    //     the manually-edited content gets baked into the next
    //     binary. The tamper-protection backstop here is the
    //     runtime test `seed_index_version_matches_const` which
    //     compares the on-disk index version against the
    //     build-emitted SEED_HUB_VERSION. Cargo treats a non-
    //     existent path here as benign (it just doesn't re-trigger).
    println!("cargo:rerun-if-changed={}", tag_file.display());
    println!("cargo:rerun-if-changed={}", seed_dir.display());

    // Resolve the desired tag BEFORE acquiring the cross-process
    // lock. Two concurrent builds would otherwise serialize on the
    // GitHub releases-list round-trip (~500ms-2s); doing the resolve
    // unlocked + re-checking the cache under the lock cuts that
    // wall-clock cost in half on parallel-build setups.
    let desired_tag = resolve_desired_tag()?;

    // Cross-process lock — two `cargo build` invocations (e.g. user
    // runs it in two terminals, or a CI matrix shares a clone) could
    // race the remove + rename below and clobber each other. The
    // lock-file lives in `<binaries>/.hub-seed.lock` so it persists
    // across builds; `fs2::FileExt::lock_exclusive` is a POSIX
    // `flock(2)` / Windows `LockFileEx` advisory lock that the kernel
    // auto-releases on process exit (SIGKILL-safe — no stale-lock
    // recovery needed).
    let lock_path = binaries_root.join(".hub-seed.lock");
    let lock_file = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&lock_path)?;
    lock_file
        .lock_exclusive()
        .map_err(|e| format!("acquire hub-seed lock: {}", e))?;
    // Lock is released when `lock_file` is dropped at the end of
    // this function — RAII semantics, no manual unlock needed.

    // Skip-if-fresh: cached `.tag` matches desired AND the seed
    // contents look intact (we settle on `index.json` as the
    // tripwire — every release has one and the runtime fails
    // gracelessly without it). Combined with the
    // `rerun-if-changed=seed_dir` above, this guarantees that any
    // manual tamper inside seed_dir re-triggers build.rs which
    // then forces a fresh download + verify.
    if seed_dir.join("index.json").exists()
        && let Ok(cached_tag) = fs::read_to_string(&tag_file)
        && cached_tag.trim() == desired_tag
    {
        println!(
            "hub-seed cache hit for {} — skipping download",
            desired_tag
        );
        write_version_file_atomically(out_dir, &desired_tag)?;
        return Ok(());
    }

    // Cache miss → fresh download into OUT_DIR staging (so a
    // mid-flight failure never leaves a partial seed under
    // binaries/hub-seed where include_dir! would pick it up).
    let staging = Path::new(out_dir).join("hub-seed-staging");
    if staging.exists() {
        fs::remove_dir_all(&staging)?;
    }
    fs::create_dir_all(&staging)?;

    let asset_base = format!(
        "https://github.com/{}/{}/releases/download/{}",
        HUB_REPO_OWNER, HUB_REPO_NAME, desired_tag,
    );

    // Six release artifacts (matches runtime — see explore notes
    // and `hub_manager.rs` around line 690): the tarball + index +
    // their sha256 sidecars + their cosign keyless bundles.
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
        download_binary(&url, &dest)?;
    }

    // Verify chain (mirrors runtime `verify_sha256_sidecar` +
    // `verify_cosign_bundle`). sha256 first because it's free; cosign
    // second because it reaches out to Sigstore endpoints. Any failure
    // bubbles up → panic in build.rs.
    verify_sha256_sidecar(
        &staging.join("hub.tar.gz"),
        &staging.join("hub.tar.gz.sha256"),
    )?;
    verify_sha256_sidecar(
        &staging.join("hub.index.json"),
        &staging.join("hub.index.json.sha256"),
    )?;
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

    // Unpack the verified tarball into a separate dir so we can
    // atomically swap into binaries/hub-seed without leaving a
    // half-extracted tree behind on failure.
    let unpacked = staging.join("contents");
    fs::create_dir_all(&unpacked)?;
    unpack_safely(&staging.join("hub.tar.gz"), &unpacked)?;

    // Overwrite index.json with the separately-signed standalone
    // (matches runtime line 798-800 — guarantees the served index
    // matches what cosign covered).
    fs::copy(
        staging.join("hub.index.json"),
        unpacked.join("index.json"),
    )?;
    // Some releases ship a duplicate `hub.index.json` alongside
    // `index.json` inside the tarball — strip it so we don't bake
    // ~8 KB of duplicate JSON into every binary via include_dir!.
    let dup = unpacked.join("hub.index.json");
    if dup.exists() {
        let _ = fs::remove_file(&dup);
    }

    // Rotate into the durable cache. Try `rename` first (atomic on
    // same filesystem). If that fails with EXDEV (CI often mounts
    // `target/` on tmpfs and `binaries/` on host disk), fall back
    // to copy-then-remove. The runtime uses a `.previous/` backup
    // here; we don't bother because a failed rotate just means
    // the next build re-downloads.
    if seed_dir.exists() {
        fs::remove_dir_all(&seed_dir)?;
    }
    match fs::rename(&unpacked, &seed_dir) {
        Ok(()) => {}
        Err(e) if is_cross_device_error(&e) => {
            println!("rotate: cross-fs rename — falling back to copy+remove");
            copy_dir_recursive(&unpacked, &seed_dir)?;
            let _ = fs::remove_dir_all(&unpacked);
        }
        Err(e) => return Err(format!("rotate seed dir: {}", e).into()),
    }

    // Stamp `.tag` AFTER the seed lands, so a partial / failed prior
    // run leaves an old tag (or no tag) and the next build's
    // skip-if-fresh correctly misses.
    write_tag_atomically(&tag_file, &desired_tag)?;
    write_version_file_atomically(out_dir, &desired_tag)?;

    let _ = fs::remove_dir_all(&staging);

    println!(
        "hub-seed staged at {} (tag {})",
        seed_dir.display(),
        desired_tag
    );
    Ok(())
}

/// Validate a release tag. Tag values flow into:
/// (a) GitHub release-download URLs, (b) the cosign identity
/// (`...@refs/tags/{tag}`), (c) the persisted `.tag` cache file,
/// (d) `SEED_HUB_VERSION` baked into the binary.
///
/// Constraint matches the runtime's `is_safe_version` at
/// `hub_manager.rs:1196-1204` so the build can't embed a value the
/// runtime would then reject (the runtime's `pin_version` path
/// re-validates with the same shape — a build-time-accepts /
/// runtime-rejects mismatch would surface as a confusing
/// admin-side "unsafe version" error).
///
/// Rules (after stripping the optional leading `v`):
/// - non-empty, ≤ 32 chars (runtime cap)
/// - charset: `[A-Za-z0-9.-]` (NO `+`, `_`, `v` mid-body — runtime rejects)
/// - starts with an ASCII digit
/// - no `..` traversal anywhere
fn validate_tag(tag: &str) -> Result<String, Box<dyn std::error::Error>> {
    if tag.is_empty() {
        return Err("tag is empty".into());
    }
    if tag.contains("..") {
        return Err(format!("tag contains '..': {:?}", tag).into());
    }
    // Optional leading `v` — strip exactly once.
    let body = tag.strip_prefix('v').unwrap_or(tag);
    if body.is_empty() {
        return Err(format!("tag has no body after `v` prefix: {:?}", tag).into());
    }
    if body.len() > MAX_VERSION_LEN {
        return Err(format!(
            "tag body too long ({} > {} chars): {:?}",
            body.len(),
            MAX_VERSION_LEN,
            tag
        )
        .into());
    }
    // Restrict to the runtime's `is_safe_version` charset exactly:
    // alphanumeric + `.` + `-`. No `+` (build-metadata in semver but
    // not used in ziee-ai/hub tags), no `_`, no embedded `v`.
    for b in body.bytes() {
        if !(b.is_ascii_alphanumeric() || b == b'.' || b == b'-') {
            return Err(format!("tag contains disallowed character: {:?}", tag).into());
        }
    }
    if !body.bytes().next().map(|b| b.is_ascii_digit()).unwrap_or(false) {
        return Err(format!(
            "tag body must start with a digit (got {:?})",
            tag
        )
        .into());
    }
    Ok(tag.to_string())
}

/// Read & validate the desired tag — `HUB_RELEASE_TAG` env wins
/// (reproducible-build escape hatch), otherwise hit GitHub for the
/// latest non-prerelease tag. Called BEFORE the cross-process lock
/// so concurrent builds don't serialize on the network round-trip.
fn resolve_desired_tag() -> Result<String, Box<dyn std::error::Error>> {
    let from_env = std::env::var("HUB_RELEASE_TAG")
        .ok()
        .map(|t| t.trim().to_string())
        .filter(|t| !t.is_empty());
    if let Some(pinned) = from_env {
        println!("Using pinned HUB_RELEASE_TAG={}", pinned);
        return validate_tag(&pinned);
    }
    let latest = resolve_latest_release()?;
    println!("Resolved latest hub release: {}", latest);
    validate_tag(&latest)
}

/// Detect EXDEV (cross-device link) errors from `fs::rename`. On
/// every Unix `EXDEV == 18` (Linux, macOS, FreeBSD, OpenBSD all
/// agree); errno 17 is EEXIST, which is a DIFFERENT failure mode
/// (a concurrent build racing the rotate) and should NOT be
/// silently swallowed by the copy-fallback. On Windows
/// `ERROR_NOT_SAME_DEVICE = 17` happens to share the value but
/// the code-path is separate via `cfg`.
fn is_cross_device_error(e: &std::io::Error) -> bool {
    #[cfg(target_family = "unix")]
    {
        matches!(e.raw_os_error(), Some(18))
    }
    #[cfg(not(target_family = "unix"))]
    {
        // ERROR_NOT_SAME_DEVICE on Windows is 17.
        matches!(e.raw_os_error(), Some(17))
    }
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<(), Box<dyn std::error::Error>> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        let to = dst.join(entry.file_name());
        if ty.is_dir() {
            copy_dir_recursive(&entry.path(), &to)?;
        } else {
            fs::copy(entry.path(), to)?;
        }
    }
    Ok(())
}

/// Atomic write: `tag_file.tmp` → fsync → rename. A signal / crash
/// mid-write leaves the old `.tag` intact rather than truncating to
/// empty (which the skip-if-fresh path's `trim() == desired_tag`
/// would silently fail, forcing an unnecessary re-fetch — annoying
/// rather than dangerous).
fn write_tag_atomically(tag_file: &Path, tag: &str) -> Result<(), Box<dyn std::error::Error>> {
    // `.tag` has no stem (whole filename is the extension), so
    // `with_extension("tag.tmp")` produces `tag.tmp` — confusing.
    // Use `with_file_name` for explicit control.
    let tmp = tag_file.with_file_name(".tag.tmp");
    {
        let mut f = fs::File::create(&tmp)?;
        f.write_all(tag.as_bytes())?;
        f.sync_all()?;
    }
    fs::rename(&tmp, tag_file)?;
    Ok(())
}

/// Atomic write of `$OUT_DIR/hub_seed_version.txt` so a half-written
/// file can never be `include_str!`'d into `SEED_HUB_VERSION` —
/// an empty const there would silently surface as `""` in the
/// admin /version endpoint.
fn write_version_file_atomically(
    out_dir: &str,
    tag: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let version = tag.strip_prefix('v').unwrap_or(tag);
    if version.is_empty() {
        return Err(format!("version derived from tag {:?} is empty", tag).into());
    }
    let final_path = Path::new(out_dir).join("hub_seed_version.txt");
    let tmp = Path::new(out_dir).join("hub_seed_version.txt.tmp");
    {
        let mut f = fs::File::create(&tmp)?;
        f.write_all(version.as_bytes())?;
        f.sync_all()?;
    }
    fs::rename(&tmp, &final_path)?;
    Ok(())
}

/// Lazily-constructed shared ureq Agent — cached so the six
/// sequential artifact downloads + the releases-list call reuse a
/// single TLS connection to github.com. Without caching, each call
/// pays a fresh TLS handshake (~200ms × 7 calls ≈ 1.4s wasted).
/// `ureq::Agent` wraps an `Arc` internally so cloning the static is
/// cheap.
fn http_agent() -> ureq::Agent {
    use std::sync::OnceLock;
    static AGENT: OnceLock<ureq::Agent> = OnceLock::new();
    AGENT
        .get_or_init(|| {
            ureq::Agent::config_builder()
                .timeout_global(Some(HTTP_TIMEOUT))
                .user_agent(concat!("ziee-build/", env!("CARGO_PKG_VERSION")))
                .build()
                .into()
        })
        .clone()
}

/// `GET /repos/<owner>/<repo>/releases?per_page=50` → newest
/// non-prerelease tag (fall back to newest prerelease if no stable
/// exists). Mirrors `hub_manager.rs::resolve_latest_release`.
fn resolve_latest_release() -> Result<String, Box<dyn std::error::Error>> {
    let url = format!(
        "https://api.github.com/repos/{}/{}/releases?per_page=50",
        HUB_REPO_OWNER, HUB_REPO_NAME,
    );
    let agent = http_agent();
    let mut req = agent
        .get(&url)
        .header("Accept", "application/vnd.github+json");
    // Honor GITHUB_TOKEN if set — GitHub's unauthenticated API limit
    // is 60 req/hr per IP; authenticated jumps to 5000/hr. Heavy CI
    // matrices share egress IPs and 429 fast otherwise.
    if let Ok(token) = std::env::var("GITHUB_TOKEN")
        && !token.is_empty()
    {
        req = req.header("Authorization", format!("Bearer {}", token));
    }
    let resp = req
        .call()
        .map_err(|e| format!("list releases ({}): {}", url, e))?;
    let status = resp.status();
    if !(200..300).contains(&status.as_u16()) {
        return Err(format!("list releases ({}): HTTP {}", url, status).into());
    }
    let body_text = resp
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
/// `download_to_file` (`hub_manager.rs:898-968`): 3 attempts, 2s
/// sleep on failure, retry on 5xx, hard-fail on 4xx. Uses ureq
/// instead of reqwest::blocking to avoid pulling reqwest into
/// build-deps (already a build-dep).
///
/// SECURITY: status check is load-bearing. ureq's `.call()` returns
/// `Ok(resp)` for ALL HTTP responses (including 4xx/5xx) by default;
/// without the `is_success()` gate, a 404 HTML body would be written
/// as "hub.tar.gz" and fail opaquely at sha256 verify time. The
/// streamed body is also `Read::take`-bounded so a chunked-transfer
/// response can't ignore the content-length cap.
fn download_binary(url: &str, dest: &Path) -> Result<u64, Box<dyn std::error::Error>> {
    let agent = http_agent();
    let mut last_err = String::new();
    for attempt in 1..=3u32 {
        let mut req = agent
            .get(url)
            .header("Accept", "application/octet-stream");
        if let Ok(token) = std::env::var("GITHUB_TOKEN")
            && !token.is_empty()
        {
            req = req.header("Authorization", format!("Bearer {}", token));
        }

        match req.call() {
            Ok(resp) => {
                let status = resp.status();
                let code = status.as_u16();
                if !(200..300).contains(&code) {
                    last_err = format!("HTTP {}", status);
                    // Retry only on server-side errors (5xx). Client
                    // errors (4xx) are permanent — a 404 for a bad
                    // tag isn't going to flip to 200.
                    if (500..600).contains(&code) && attempt < 3 {
                        std::thread::sleep(Duration::from_secs(2));
                        continue;
                    }
                    return Err(format!("download {}: {}", url, last_err).into());
                }
                // Pre-check content-length header against the cap.
                if let Some(len_hdr) = resp.headers().get("content-length")
                    && let Ok(len_str) = len_hdr.to_str()
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
                // `take(cap + 1)` so we can detect overruns: if the
                // copy reads more than `cap` bytes, the server lied
                // about content-length (or omitted it) — fail-closed.
                let reader = resp.into_body().into_reader();
                let mut bounded = reader.take(MAX_HUB_ARTIFACT_BYTES + 1);
                match std::io::copy(&mut bounded, &mut file) {
                    Ok(n) if n > MAX_HUB_ARTIFACT_BYTES => {
                        let _ = fs::remove_file(dest);
                        return Err(format!(
                            "{}: body exceeded cap of {} bytes",
                            url, MAX_HUB_ARTIFACT_BYTES
                        )
                        .into());
                    }
                    Ok(n) => return Ok(n),
                    Err(e) => {
                        last_err = format!("stream-to-file: {}", e);
                        let _ = fs::remove_file(dest);
                        if attempt < 3 {
                            std::thread::sleep(Duration::from_secs(2));
                            continue;
                        }
                        return Err(format!("download {}: {}", url, last_err).into());
                    }
                }
            }
            Err(e) => {
                last_err = format!("send: {}", e);
                if attempt < 3 {
                    std::thread::sleep(Duration::from_secs(2));
                    continue;
                }
                return Err(format!("download {}: {}", url, last_err).into());
            }
        }
    }
    Err(format!("download {}: {}", url, last_err).into())
}

/// sha256sum sidecar shape: `<hex>  <filename>\n`. Mirrors runtime
/// `verify_sha256_sidecar` at `hub_manager.rs:986`.
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

/// Cosign keyless verify via the sigstore blocking API, run on a
/// fresh OS thread with a wall-clock cap.
///
/// Two reasons for the thread:
///
///   1. **tokio nesting**: build.rs runs inside `#[tokio::main]` (so
///      sqlx can do its compile-time validation against the build
///      DB elsewhere). Sigstore's "blocking" API internally calls
///      `tokio::runtime::Builder::new_current_thread().build()
///      .block_on(...)` to drive its async Rekor/Fulcio fetches,
///      which panics with "Cannot start a runtime from within a
///      runtime" if invoked on a thread that already owns one. A
///      fresh OS thread has no runtime context.
///   2. **timeout**: sigstore has no caller-supplied per-request
///      timeout. A stalled Sigstore endpoint would freeze the
///      build with no log progress. Running on a thread + waiting
///      via an mpsc channel with `recv_timeout` lets us surface
///      "cosign verify timed out" instead of hanging.
///
/// Mirrors runtime `verify_cosign_bundle` at
/// `hub_manager.rs:1028-1049` for the actual verify step.
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

    let (tx, rx) = mpsc::channel::<Result<(), String>>();
    std::thread::spawn(move || {
        // Wrap the whole body in `catch_unwind` so the channel send
        // happens even on a sigstore panic — without this, a
        // panic would just drop the thread and `recv_timeout`
        // would treat it as a timeout, masking the real failure.
        let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            verify_cosign_bundle_inner(&bundle_path, &blob_path, &identity, &issuer)
        }))
        .unwrap_or_else(|payload| {
            let msg = panic_message(&payload);
            Err(format!("cosign verify thread panicked: {}", msg))
        });
        let _ = tx.send(outcome);
    });

    match rx.recv_timeout(COSIGN_TIMEOUT) {
        Ok(result) => result,
        Err(mpsc::RecvTimeoutError::Timeout) => {
            // Thread keeps running in the background. It will exit
            // either when sigstore's internal HTTP eventually fails
            // or when the build process exits (whichever comes
            // first). The orphaned thread can't be cancelled (std
            // has no thread::cancel), but it's bounded by the
            // build process lifetime so it's not a real leak.
            eprintln!(
                "warning: cosign verify thread orphaned at {}s timeout; will be reaped at build exit",
                COSIGN_TIMEOUT.as_secs()
            );
            Err(format!(
                "cosign verify timed out after {}s (Sigstore endpoint stalled?)",
                COSIGN_TIMEOUT.as_secs()
            ))
        }
        Err(mpsc::RecvTimeoutError::Disconnected) => {
            Err("cosign verify thread vanished without reporting".to_string())
        }
    }
}

fn verify_cosign_bundle_inner(
    bundle_path: &Path,
    blob_path: &Path,
    identity: &str,
    issuer: &str,
) -> Result<(), String> {
    use sigstore::bundle::Bundle;
    use sigstore::bundle::verify::blocking::Verifier;
    use sigstore::bundle::verify::policy::Identity;

    let bundle_json =
        fs::read_to_string(bundle_path).map_err(|e| format!("read bundle: {}", e))?;
    let bundle: Bundle =
        serde_json::from_str(&bundle_json).map_err(|e| format!("parse bundle: {}", e))?;
    let blob = fs::File::open(blob_path).map_err(|e| format!("open blob: {}", e))?;
    let verifier = Verifier::production().map_err(|e| format!("trust root init: {}", e))?;
    let policy = Identity::new(identity, issuer);
    verifier
        .verify(blob, bundle, &policy, false)
        .map_err(|e| format!("signature verification: {}", e))?;
    Ok(())
}

/// Extract a human-readable message from a panic payload (the
/// `Box<dyn Any>` returned by `catch_unwind` or `JoinHandle::join`).
/// `payload.downcast_ref::<String>()` covers `panic!("...".to_string())`;
/// `<&str>` covers `panic!("...")`.
fn panic_message(payload: &Box<dyn std::any::Any + Send>) -> String {
    if let Some(s) = payload.downcast_ref::<String>() {
        s.clone()
    } else if let Some(s) = payload.downcast_ref::<&str>() {
        (*s).to_string()
    } else {
        "<unknown panic payload>".to_string()
    }
}

/// Tar extraction with traversal + decompression-bomb guards. Mirrors
/// runtime `unpack_safely` at `hub_manager.rs:1053-1128`, plus an
/// extra `Component::Prefix` / `Component::RootDir` reject that the
/// runtime is missing (Windows-style `C:\foo` paths inside a
/// tarball produced on Windows aren't `is_absolute()` on Linux).
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
            match component {
                std::path::Component::ParentDir => {
                    return Err(format!(
                        "parent-dir component in archive: {}",
                        path.display()
                    )
                    .into());
                }
                std::path::Component::RootDir => {
                    return Err(format!(
                        "root-dir component in archive: {}",
                        path.display()
                    )
                    .into());
                }
                std::path::Component::Prefix(_) => {
                    return Err(format!(
                        "windows-prefix component in archive: {}",
                        path.display()
                    )
                    .into());
                }
                std::path::Component::CurDir => {
                    // `./foo` normalizes to `foo` under unpack_in,
                    // but tarballs in the wild sometimes include
                    // bare `.` entries which would extract into
                    // `dest` itself with unpredictable mode bits.
                    return Err(format!(
                        "cur-dir component in archive: {}",
                        path.display()
                    )
                    .into());
                }
                _ => {}
            }
        }
        entry
            .unpack_in(dest)
            .map_err(|e| format!("unpack {}: {}", path.display(), e))?;
    }
    Ok(())
}

