//! Library form of the rootfs fetch flow.
//!
//! Resolves a flavor against the binary's embedded
//! `known_revisions.toml`, downloads the matching squashfs from
//! GitHub Releases (or the `CODE_SANDBOX_ROOTFS_MIRROR` override),
//! verifies sha256 + cosign signature, and atomically installs into
//! the cache dir. Idempotent: if the target file is already cached
//! with a matching sha256, returns immediately.
//!
//! The only public entry point is [`fetch_flavor`], called from
//! `runtime_mount::ensure_rootfs_ready` (auto-fetch on first use of a
//! flavor). There is no CLI fetch command — the runtime owns fetching
//! end-to-end.
//!
//! Internal blocking work (reqwest::blocking, sigstore::blocking) runs
//! inside `tokio::task::spawn_blocking` so it doesn't panic dropping
//! its internal runtimes from within the outer `#[tokio::main]` context.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use crate::core::config::CodeSandboxConfig;
use crate::modules::code_sandbox::{
    SANDBOX_KNOWN_REVISIONS_TOML, SANDBOX_ROOTFS_SCHEMA_VERSION,
};

// =====================================================================
// Public surface
// =====================================================================

/// Streamed progress event from `fetch_flavor`. Wrapped in a callback
/// so the caller can decide how to surface it (chat UI system note,
/// tracing log, structured response field, …).
#[derive(Debug, Clone)]
pub struct FetchProgress {
    pub phase: FetchPhase,
    /// Free-form human-readable message ("Downloading…", "Verifying
    /// sha256…", "cosign OK"). The chat UI may choose to display this
    /// verbatim or aggregate by phase.
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum FetchPhase {
    Resolving,
    Downloading,
    VerifyingSha256,
    VerifyingCosign,
    Installing,
}

#[derive(Debug, Clone)]
pub struct FetchOutcome {
    pub installed_path: PathBuf,
    pub bytes_downloaded: u64,
    pub duration_ms: u64,
    pub cosign_verified: bool,
}

#[derive(Debug, Clone)]
pub enum FetchError {
    EmptyKnownRevisions,
    InvalidKnownRevisions(String),
    UnknownFlavor {
        flavor: String,
        available: Vec<String>,
    },
    SchemaMismatch {
        found: u32,
        expected: u32,
    },
    MalformedSha256(String),
    MirrorMustBeHttps {
        url: String,
    },
    Download(String),
    Sha256Mismatch {
        expected: String,
        got: String,
    },
    CosignBundleMissing {
        url: String,
    },
    CosignFailed(String),
    Install(String),
}

impl std::fmt::Display for FetchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FetchError::EmptyKnownRevisions => write!(
                f,
                "embedded known_revisions.toml is empty (no published releases yet)"
            ),
            FetchError::InvalidKnownRevisions(e) => write!(f, "invalid known_revisions.toml: {e}"),
            FetchError::UnknownFlavor { flavor, available } => write!(
                f,
                "no known revision for flavor {flavor:?} (available: {available:?})"
            ),
            FetchError::SchemaMismatch { found, expected } => write!(
                f,
                "rootfs schema v{found} but this binary expects v{expected}"
            ),
            FetchError::MalformedSha256(s) => write!(f, "malformed sha256 in known_revisions: {s}"),
            FetchError::MirrorMustBeHttps { url } => {
                write!(f, "CODE_SANDBOX_ROOTFS_MIRROR must be https:// (got {url:?})")
            }
            FetchError::Download(e) => write!(f, "download failed: {e}"),
            FetchError::Sha256Mismatch { expected, got } => {
                write!(f, "sha256 mismatch (expected {expected}, got {got})")
            }
            FetchError::CosignBundleMissing { url } => {
                write!(f, "signed=true revision has no cosign bundle at {url}")
            }
            FetchError::CosignFailed(e) => write!(f, "cosign verification failed: {e}"),
            FetchError::Install(e) => write!(f, "atomic install failed: {e}"),
        }
    }
}

/// Resolve, download, verify, and install the squashfs for `flavor`
/// matching this binary's schema + current arch. Always resolves to
/// the LATEST non-yanked revision (callers wanting an exact pin
/// should call `fetch_revision` instead — kept private until the CLI
/// path is fully removed).
///
/// Idempotent: if the target file is already in `cache_dir` AND its
/// sha256 matches the embedded `known_revisions.toml`, returns
/// `Ok` with `bytes_downloaded = 0` without touching the network.
pub async fn fetch_flavor(
    cache_dir: &Path,
    flavor: &str,
    progress: impl Fn(FetchProgress) + Send + Sync + 'static,
) -> Result<FetchOutcome, FetchError> {
    let cache_dir = cache_dir.to_path_buf();
    let flavor = flavor.to_string();
    tokio::task::spawn_blocking(move || {
        fetch_blocking(&cache_dir, "latest", &flavor, std::env::consts::ARCH, &progress)
    })
    .await
    .unwrap_or_else(|e| Err(FetchError::Install(format!("blocking task panicked: {e}"))))
}

/// Enumerate flavors known to this binary for the current arch.
/// Used by `list_sandbox_environments` (MCP tool, Phase 5).
pub fn available_flavors(_cfg: &CodeSandboxConfig) -> Vec<String> {
    let arch = std::env::consts::ARCH;
    let known = match parse_known_revisions_toml() {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };
    let entries: Vec<&toml::value::Table> = known
        .get("revision")
        .and_then(|r| r.as_array())
        .map(|arr| arr.iter().filter_map(|v| v.as_table()).collect())
        .unwrap_or_default();
    let mut flavors: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    for e in entries {
        if e.get("schema").and_then(|v| v.as_integer())
            == Some(SANDBOX_ROOTFS_SCHEMA_VERSION as i64)
            && e.get("arch").and_then(|v| v.as_str()) == Some(arch)
            && !e.get("yanked").and_then(|v| v.as_bool()).unwrap_or(false)
        {
            if let Some(f) = e.get("flavor").and_then(|v| v.as_str()) {
                flavors.insert(f.to_string());
            }
        }
    }
    flavors.into_iter().collect()
}

// =====================================================================
// Internal blocking implementation (runs on spawn_blocking thread —
// safe to call reqwest::blocking + sigstore::blocking here)
// =====================================================================

fn fetch_blocking(
    cache_dir: &Path,
    version: &str,
    flavor: &str,
    arch: &str,
    progress: &(dyn Fn(FetchProgress) + Send + Sync),
) -> Result<FetchOutcome, FetchError> {
    let started = Instant::now();

    progress(FetchProgress {
        phase: FetchPhase::Resolving,
        message: format!("resolving {version} flavor={flavor} arch={arch}"),
    });

    let resolved = resolve_revision(version, flavor, arch)?;
    let (tag, asset, expected_sha, signed_required) = (
        format!("sandbox-rootfs-v{}.{}-{}", resolved.schema, resolved.revision, arch),
        format!(
            "ziee-sandbox-rootfs-v{}.{}-{}-{}.squashfs",
            resolved.schema, resolved.revision, arch, flavor
        ),
        resolved.sha256.clone(),
        resolved.signed,
    );

    std::fs::create_dir_all(cache_dir).map_err(|e| {
        FetchError::Install(format!("create cache dir {}: {e}", cache_dir.display()))
    })?;
    let out_path = cache_dir.join(&asset);
    let tmp_path = out_path.with_extension("squashfs.tmp");

    // Idempotency: if the final file is already there and its sha
    // matches, short-circuit. Lets the runtime auto-fetch path no-op
    // when an operator pre-staged the squashfs (air-gapped install).
    if out_path.exists() {
        progress(FetchProgress {
            phase: FetchPhase::VerifyingSha256,
            message: format!("cached {} present; verifying", out_path.display()),
        });
        match sha256_file(&out_path) {
            Ok(s) if s == expected_sha => {
                return Ok(FetchOutcome {
                    installed_path: out_path,
                    bytes_downloaded: 0,
                    duration_ms: started.elapsed().as_millis() as u64,
                    cosign_verified: false, // not re-verified in the cached path
                });
            }
            Ok(s) => {
                progress(FetchProgress {
                    phase: FetchPhase::Resolving,
                    message: format!(
                        "cached file sha mismatch (expected {expected_sha}, got {s}); re-downloading"
                    ),
                });
                let _ = std::fs::remove_file(&out_path);
            }
            Err(_) => {
                let _ = std::fs::remove_file(&out_path);
            }
        }
    }

    let url = build_download_url(&tag, &asset)?;

    progress(FetchProgress {
        phase: FetchPhase::Downloading,
        message: format!("downloading {url}"),
    });
    let bytes_downloaded = match download_to_file(&url, &tmp_path, 3) {
        DownloadResult::Ok(n) => n,
        DownloadResult::NotFound => {
            return Err(FetchError::Download(format!("HTTP 404 at {url}")));
        }
        DownloadResult::Failed(e) => {
            let _ = std::fs::remove_file(&tmp_path);
            return Err(FetchError::Download(e));
        }
    };

    progress(FetchProgress {
        phase: FetchPhase::VerifyingSha256,
        message: "verifying sha256".to_string(),
    });
    let actual_sha = sha256_file(&tmp_path).map_err(|e| {
        let _ = std::fs::remove_file(&tmp_path);
        FetchError::Install(format!("sha256 read: {e}"))
    })?;
    if actual_sha != expected_sha {
        let _ = std::fs::remove_file(&tmp_path);
        return Err(FetchError::Sha256Mismatch {
            expected: expected_sha,
            got: actual_sha,
        });
    }

    // Cosign verification. Skipped if `signed = false` AND bundle
    // download 404s; required (fail-closed) if `signed = true`.
    let cosign_verified = verify_cosign_step(
        &url,
        &out_path,
        &tmp_path,
        signed_required,
        resolved.schema,
        &resolved.revision,
        arch,
        progress,
    )?;

    progress(FetchProgress {
        phase: FetchPhase::Installing,
        message: format!("installing {}", out_path.display()),
    });
    std::fs::rename(&tmp_path, &out_path).map_err(|e| {
        let _ = std::fs::remove_file(&tmp_path);
        FetchError::Install(format!("rename to {}: {e}", out_path.display()))
    })?;

    Ok(FetchOutcome {
        installed_path: out_path,
        bytes_downloaded,
        duration_ms: started.elapsed().as_millis() as u64,
        cosign_verified,
    })
}

#[allow(clippy::too_many_arguments)]
fn verify_cosign_step(
    asset_url: &str,
    out_path: &Path,
    tmp_path: &Path,
    signed_required: bool,
    schema: i64,
    revision: &str,
    arch: &str,
    progress: &(dyn Fn(FetchProgress) + Send + Sync),
) -> Result<bool, FetchError> {
    let bundle_url = format!("{asset_url}.cosign.bundle");
    let bundle_path = out_path.with_extension("squashfs.cosign.bundle");

    progress(FetchProgress {
        phase: FetchPhase::VerifyingCosign,
        message: format!("downloading cosign bundle from {bundle_url}"),
    });
    let bundle_present = match download_to_file(&bundle_url, &bundle_path, 2) {
        DownloadResult::Ok(_) => bundle_path.exists(),
        DownloadResult::NotFound => false,
        DownloadResult::Failed(e) => {
            progress(FetchProgress {
                phase: FetchPhase::VerifyingCosign,
                message: format!("(cosign bundle download failed: {e})"),
            });
            false
        }
    };

    if !bundle_present {
        if signed_required {
            let _ = std::fs::remove_file(tmp_path);
            return Err(FetchError::CosignBundleMissing { url: bundle_url });
        }
        progress(FetchProgress {
            phase: FetchPhase::VerifyingCosign,
            message: "(no cosign bundle published; sha256-only)".to_string(),
        });
        return Ok(false);
    }

    progress(FetchProgress {
        phase: FetchPhase::VerifyingCosign,
        message: "verifying cosign signature".to_string(),
    });
    let expected_identity = format!(
        "https://github.com/phibya/ziee-chat/.github/workflows/\
         code_sandbox.yml@refs/tags/sandbox-rootfs-v{schema}.{revision}-{arch}"
    );
    verify_cosign_bundle(
        &bundle_path,
        tmp_path,
        &expected_identity,
        "https://token.actions.githubusercontent.com",
    )
    .map_err(|e| {
        let _ = std::fs::remove_file(tmp_path);
        let _ = std::fs::remove_file(&bundle_path);
        FetchError::CosignFailed(e)
    })?;
    Ok(true)
}

// =====================================================================
// known_revisions.toml parsing + revision resolution
// =====================================================================

struct Resolved {
    schema: i64,
    revision: String,
    sha256: String,
    signed: bool,
}

fn parse_known_revisions_toml() -> Result<toml::Value, FetchError> {
    let revisions_toml: std::borrow::Cow<'_, str> = if cfg!(debug_assertions) {
        match std::env::var("CODE_SANDBOX_KNOWN_REVISIONS_OVERRIDE") {
            Ok(p) => match std::fs::read_to_string(&p) {
                Ok(s) => {
                    tracing::warn!("code_sandbox: dev override known_revisions: {p}");
                    s.into()
                }
                Err(e) => {
                    return Err(FetchError::InvalidKnownRevisions(format!(
                        "CODE_SANDBOX_KNOWN_REVISIONS_OVERRIDE={p}: {e}"
                    )));
                }
            },
            Err(_) => SANDBOX_KNOWN_REVISIONS_TOML.into(),
        }
    } else {
        SANDBOX_KNOWN_REVISIONS_TOML.into()
    };
    toml::from_str(&revisions_toml).map_err(|e| FetchError::InvalidKnownRevisions(e.to_string()))
}

fn resolve_revision(version: &str, flavor: &str, arch: &str) -> Result<Resolved, FetchError> {
    let known = parse_known_revisions_toml()?;
    let entries: Vec<&toml::value::Table> = known
        .get("revision")
        .and_then(|r| r.as_array())
        .map(|arr| arr.iter().filter_map(|v| v.as_table()).collect())
        .unwrap_or_default();
    if entries.is_empty() {
        return Err(FetchError::EmptyKnownRevisions);
    }

    fn revision_number(rev: &str) -> Option<u32> {
        rev.strip_prefix('r').and_then(|n| n.parse().ok())
    }
    fn is_yanked(e: &toml::value::Table) -> bool {
        e.get("yanked").and_then(|v| v.as_bool()).unwrap_or(false)
    }

    let resolved_tbl = if version == "latest" {
        let mut candidates: Vec<&toml::value::Table> = entries
            .iter()
            .copied()
            .filter(|e| {
                !is_yanked(e)
                    && e.get("schema").and_then(|v| v.as_integer())
                        == Some(SANDBOX_ROOTFS_SCHEMA_VERSION as i64)
                    && e.get("arch").and_then(|v| v.as_str()) == Some(arch)
                    && e.get("flavor").and_then(|v| v.as_str()) == Some(flavor)
            })
            .collect();
        candidates.sort_by_key(|e| {
            e.get("revision")
                .and_then(|v| v.as_str())
                .and_then(revision_number)
                .unwrap_or(0)
        });
        candidates.last().copied().ok_or_else(|| FetchError::UnknownFlavor {
            flavor: flavor.to_string(),
            available: enumerate_flavors_for_schema_arch(&entries, arch),
        })?
    } else {
        let v = version.strip_prefix('v').unwrap_or(version);
        let mut parts = v.splitn(2, '.');
        let schema: i64 = parts
            .next()
            .and_then(|s| s.parse().ok())
            .ok_or_else(|| FetchError::Install(format!("invalid version {version:?}")))?;
        let revision = parts
            .next()
            .ok_or_else(|| FetchError::Install(format!("invalid version {version:?}")))?
            .to_string();
        entries
            .iter()
            .copied()
            .find(|e| {
                e.get("schema").and_then(|v| v.as_integer()) == Some(schema)
                    && e.get("revision").and_then(|v| v.as_str()) == Some(&revision)
                    && e.get("arch").and_then(|v| v.as_str()) == Some(arch)
                    && e.get("flavor").and_then(|v| v.as_str()) == Some(flavor)
            })
            .ok_or_else(|| FetchError::UnknownFlavor {
                flavor: flavor.to_string(),
                available: enumerate_flavors_for_schema_arch(&entries, arch),
            })?
    };

    let schema = resolved_tbl
        .get("schema")
        .and_then(|v| v.as_integer())
        .unwrap_or(0);
    let revision = resolved_tbl
        .get("revision")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    if schema != SANDBOX_ROOTFS_SCHEMA_VERSION as i64 {
        return Err(FetchError::SchemaMismatch {
            found: schema as u32,
            expected: SANDBOX_ROOTFS_SCHEMA_VERSION,
        });
    }
    let sha = resolved_tbl
        .get("sha256")
        .and_then(|v| v.as_str())
        .map(|s| s.trim().to_lowercase())
        .ok_or_else(|| FetchError::MalformedSha256("missing".to_string()))?;
    if sha.len() != 64 || !sha.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(FetchError::MalformedSha256(sha));
    }
    let signed = resolved_tbl
        .get("signed")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    Ok(Resolved { schema, revision, sha256: sha, signed })
}

fn enumerate_flavors_for_schema_arch(
    entries: &[&toml::value::Table],
    arch: &str,
) -> Vec<String> {
    let mut set: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    for e in entries {
        if e.get("schema").and_then(|v| v.as_integer())
            == Some(SANDBOX_ROOTFS_SCHEMA_VERSION as i64)
            && e.get("arch").and_then(|v| v.as_str()) == Some(arch)
        {
            if let Some(f) = e.get("flavor").and_then(|v| v.as_str()) {
                set.insert(f.to_string());
            }
        }
    }
    set.into_iter().collect()
}

// =====================================================================
// URL construction (https-only, with debug-only loopback http allowance)
// =====================================================================

fn build_download_url(tag: &str, asset: &str) -> Result<String, FetchError> {
    let base_url = std::env::var("CODE_SANDBOX_ROOTFS_MIRROR")
        .unwrap_or_else(|_| "https://github.com/phibya/ziee-chat/releases/download".to_string());
    let is_dev_loopback = cfg!(debug_assertions)
        && (base_url.starts_with("http://127.0.0.1")
            || base_url.starts_with("http://localhost")
            || base_url.starts_with("http://[::1]"));
    if !base_url.starts_with("https://") && !is_dev_loopback {
        return Err(FetchError::MirrorMustBeHttps { url: base_url });
    }
    if is_dev_loopback {
        tracing::warn!("code_sandbox: using http:// loopback mirror (debug build only)");
    } else if std::env::var("CODE_SANDBOX_ROOTFS_MIRROR").is_ok() {
        tracing::warn!("code_sandbox: using mirror {base_url}");
    }
    Ok(format!("{base_url}/{tag}/{asset}"))
}

// =====================================================================
// Download (reqwest::blocking — runs on this thread, no nested runtime)
// =====================================================================

enum DownloadResult {
    /// (bytes written)
    Ok(u64),
    NotFound,
    Failed(String),
}

fn download_to_file(url: &str, dest: &Path, attempts: u32) -> DownloadResult {
    // We're on a tokio::spawn_blocking thread — NOT inside an outer
    // tokio runtime context. reqwest::blocking can build + drop its
    // internal current-thread runtime freely here.
    let client = match reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(600))
        .build()
    {
        Ok(c) => c,
        Err(e) => return DownloadResult::Failed(format!("client build: {e}")),
    };

    let mut last_err = String::new();
    for attempt in 1..=attempts {
        match client.get(url).send() {
            Ok(resp) => {
                let status = resp.status();
                if status == reqwest::StatusCode::NOT_FOUND {
                    return DownloadResult::NotFound;
                }
                if !status.is_success() {
                    last_err = format!("HTTP {status}");
                    if status.is_server_error() && attempt < attempts {
                        std::thread::sleep(Duration::from_secs(2));
                        continue;
                    }
                    return DownloadResult::Failed(last_err);
                }
                let mut file = match std::fs::File::create(dest) {
                    Ok(f) => f,
                    Err(e) => {
                        return DownloadResult::Failed(format!(
                            "create {}: {e}",
                            dest.display()
                        ))
                    }
                };
                let mut resp = resp;
                match resp.copy_to(&mut file) {
                    Ok(n) => return DownloadResult::Ok(n),
                    Err(e) => {
                        last_err = format!("stream-to-file: {e}");
                        let _ = std::fs::remove_file(dest);
                        if attempt < attempts {
                            std::thread::sleep(Duration::from_secs(2));
                            continue;
                        }
                        return DownloadResult::Failed(last_err);
                    }
                }
            }
            Err(e) => {
                last_err = format!("send: {e}");
                if attempt < attempts {
                    std::thread::sleep(Duration::from_secs(2));
                    continue;
                }
                return DownloadResult::Failed(last_err);
            }
        }
    }
    DownloadResult::Failed(last_err)
}

// =====================================================================
// sha256 (streams; doesn't load whole file into memory)
// =====================================================================

fn sha256_file(path: &Path) -> std::io::Result<String> {
    use sha2::{Digest, Sha256};
    use std::io::Read;
    let mut f = std::fs::File::open(path)?;
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

// =====================================================================
// Cosign keyless OIDC verification (sigstore crate)
// =====================================================================

fn verify_cosign_bundle(
    bundle_path: &Path,
    blob_path: &Path,
    identity: &str,
    issuer: &str,
) -> Result<(), String> {
    use sigstore::bundle::verify::blocking::Verifier;
    use sigstore::bundle::verify::policy::Identity;
    use sigstore::bundle::Bundle;

    let bundle_json =
        std::fs::read_to_string(bundle_path).map_err(|e| format!("read bundle: {e}"))?;
    let bundle: Bundle =
        serde_json::from_str(&bundle_json).map_err(|e| format!("parse bundle: {e}"))?;
    let blob = std::fs::File::open(blob_path).map_err(|e| format!("open blob: {e}"))?;
    let verifier = Verifier::production().map_err(|e| format!("trust root init: {e}"))?;
    let policy = Identity::new(identity, issuer);
    verifier
        .verify(blob, bundle, &policy, false)
        .map_err(|e| format!("signature verification: {e}"))?;
    Ok(())
}
