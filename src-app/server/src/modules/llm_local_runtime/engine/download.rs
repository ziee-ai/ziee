//! Binary downloader for engine executables from GitHub releases
//!
//! Downloads pre-built engine binaries from GitHub releases with:
//! - Progress bars
//! - Resume support for interrupted downloads
//! - Automatic caching in ~/.llm-runtime/binaries/
//! - Executable permission setting (Unix)

use super::error::{Result, RuntimeError};
use super::types::EngineType;
use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};

/// Base host for release-artifact downloads.
///
/// Defaults to `https://github.com`. In **debug builds only** the
/// `LLM_RUNTIME_RELEASE_MIRROR` env var may override it so integration
/// tests can serve a stub engine from a loopback mock release server
/// (mirrors code_sandbox's `CODE_SANDBOX_ROOTFS_MIRROR`). The env read is
/// compiled out of release builds via `cfg!(debug_assertions)`, so the
/// production binary always points at the real GitHub host.
fn release_base_url() -> String {
    #[cfg(debug_assertions)]
    if let Ok(mirror) = std::env::var("LLM_RUNTIME_RELEASE_MIRROR") {
        let mirror = mirror.trim_end_matches('/');
        if !mirror.is_empty() {
            return mirror.to_string();
        }
    }
    "https://github.com".to_string()
}

/// Base host for the GitHub API (used to resolve `latest` → a tag).
///
/// Defaults to `https://api.github.com`; debug-only override via
/// `LLM_RUNTIME_API_MIRROR`. Same compile-out rules as
/// [`release_base_url`]. Most tests pass an explicit version and never
/// hit this path.
fn api_base_url() -> String {
    #[cfg(debug_assertions)]
    if let Ok(mirror) = std::env::var("LLM_RUNTIME_API_MIRROR") {
        let mirror = mirror.trim_end_matches('/');
        if !mirror.is_empty() {
            return mirror.to_string();
        }
    }
    "https://api.github.com".to_string()
}

/// The operator's GitHub token, if one is set.
///
/// Unauthenticated GitHub API access is capped at **60 requests/hour/IP**; a
/// token raises it to 5000/hour. Returns `None` for an unset or blank value so
/// an empty env var can never produce an `Authorization: Bearer ` header.
///
/// **Emptiness is the only thing filtered here, deliberately.** A non-empty but
/// INVALID value — a placeholder, a typo, an expired or revoked PAT — is
/// forwarded verbatim, because a token's validity cannot be told from its
/// string: GitHub has several valid formats (`ghp_`, `github_pat_`, `ghs_`,
/// `gho_`, …) and adds more, so a shape check would reject valid credentials
/// and would still miss an expired one. Validity is judged from GitHub's
/// RESPONSE instead — see [`is_auth_rejection`] and the anonymous fallback in
/// [`BinaryDownloader::github_get_with_retry`], which is what stops a bad token
/// from failing WORSE than no token at all.
///
/// The value is returned for immediate use in a header and is never logged,
/// never placed in an error string, and never serialized onto a response.
fn github_token() -> Option<String> {
    std::env::var("GITHUB_TOKEN")
        .ok()
        .map(|t| t.trim().to_string())
        .filter(|t| !t.is_empty())
}

/// True when the credential may be presented to `url`.
///
/// The token is an operator secret, and `api_base_url()` is overridable in
/// debug builds via `LLM_RUNTIME_API_MIRROR`. Without this guard, any debug
/// process that has a REAL `GITHUB_TOKEN` in its environment and a mirror
/// configured would transmit that credential to an arbitrary host, in
/// cleartext over http. `cfg!(debug_assertions)` alone is not the boundary:
/// it gates whether the mirror is READ, not where it points.
///
/// So the credential goes to the real GitHub host, or — in debug only — to a
/// LOOPBACK mirror, which is the integration suite's own mock and cannot
/// exfiltrate anything. A debug mirror pointed anywhere else still works; it
/// simply gets anonymous requests.
fn credential_target_is_trusted(url: &str) -> bool {
    // Parse, never string-surgery. A hand-rolled `split('/')` +
    // `rsplit_once(':')` authority parser walks straight through
    // `http://localhost:8080@evil.example` — it reads the userinfo as the host
    // and `8080@evil.example` as the port — and would hand the operator's real
    // token to `evil.example` in cleartext. That is the exact scenario this
    // function exists to prevent, so it must use a real URL parser.
    let Ok(parsed) = reqwest::Url::parse(url) else {
        return false;
    };
    let Some(host) = parsed.host_str() else {
        return false;
    };
    // `host_str()` is already lowercased and userinfo-free, and IPv6 literals
    // arrive without brackets. A trailing root dot names the same host.
    let host = host.trim_end_matches('.');

    if parsed.scheme() == "https" && host == "api.github.com" {
        return true;
    }

    // The debug-only loopback seam the integration suite drives. Release builds
    // compile this away, so production can only ever authenticate to GitHub.
    #[cfg(debug_assertions)]
    if matches!(parsed.scheme(), "http" | "https")
        && matches!(host, "127.0.0.1" | "::1" | "localhost")
    {
        return true;
    }

    false
}

/// The health of the GitHub credential a catalogue read was made with.
///
/// This is a SECOND, orthogonal axis to
/// [`super::release_cache::CatalogSource`]: `CatalogSource` says where the
/// catalogue came from, this says whether the operator's credential worked.
/// They are independent — an anonymous-rescued read is genuinely `Live` while
/// its credential is `Rejected` — which is exactly why this could not be folded
/// into `source` or signalled by setting `unavailable_reason` (the UI derives
/// "couldn't reach GitHub" from the latter, and that would be a false claim
/// over a card that is happily listing versions).
///
/// Without it, "GitHub is down" and "your token is wrong" are indistinguishable
/// to the operator, and the working anonymous path they had before pasting a
/// bad token is silently gone.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CredentialStatus {
    /// No `GITHUB_TOKEN` was configured; the request was anonymous by design.
    /// Not a problem — it simply means the 60 requests/hour/IP budget applies.
    Absent,
    /// A token was configured and GitHub accepted it (5000 requests/hour).
    ///
    /// Only reachable once GitHub has actually ANSWERED with something other
    /// than a rejection. It is a claim about what upstream DID, so it must
    /// never be reported for a request upstream never answered.
    Used,
    /// A token was configured and presented, but GitHub never SERVED the
    /// request, so its validity is unknown. Reached both when nothing came
    /// back at all (transport failure) and when what came back proves nothing:
    /// a bare `403`, a `404`, a persistent `5xx`, a throttle.
    ///
    /// Exists because the honest answer to "does my token work?" after a total
    /// outage is "we could not find out". Reporting `Used` there would assert
    /// an acceptance that never happened and tell the operator they are on the
    /// 5000/hour budget on no evidence at all.
    Unverified,
    /// A token was configured and GitHub REJECTED it; the request was re-issued
    /// anonymously. Reportable whether that retry then succeeded or not.
    Rejected,
}

impl CredentialStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Absent => "absent",
            Self::Used => "used",
            Self::Unverified => "unverified",
            Self::Rejected => "rejected",
        }
    }

    /// Text to append to a FAILING read's reason so an operator reading only
    /// `unavailable_reason` still learns their credential was the problem.
    ///
    /// Names the environment VARIABLE, never its value — the token is never
    /// logged, never placed in an error string, never serialized.
    fn failure_note(&self) -> &'static str {
        match self {
            Self::Rejected => {
                " (GitHub rejected the configured GITHUB_TOKEN, and the \
                  anonymous retry also failed — check or unset the token)"
            }
            Self::Absent | Self::Used | Self::Unverified => "",
        }
    }
}

/// True when `status` says GitHub REJECTED THE CREDENTIAL, as opposed to
/// rate-limiting or otherwise refusing an accepted one.
///
/// **`401`, plus the one `403` that identifies itself — and no other `403`.**
/// The cases are genuinely different
/// and the difference is the whole point, so it is worth stating why the
/// distinction resolves this way rather than the intuitive way:
///
/// - `401` is what GitHub answers for a bad, expired or revoked credential. It
///   is what the reported defect actually produced (`401 Bad credentials`), and
///   it means nothing except "this credential is not valid".
/// - `403` is overloaded — primary rate limit, SECONDARY rate limit, SAML
///   enforcement, missing scope, repo access — and in general it **cannot be
///   disambiguated from the headers**. `x-ratelimit-*` rides on nearly every
///   API response, so a non-zero `remaining` does not imply "not a rate limit";
///   and GitHub documents that a secondary rate limit may arrive with NO
///   `retry-after` at all. A header rule that called those rejections would
///   spend the scarce anonymous budget AND tell an operator to replace a
///   perfectly valid token.
/// - The single exception is `403` + **`X-GitHub-SSO`**, which GitHub sends
///   when a PAT has not been authorized for a SAML-SSO-enforced organization.
///   That is a documented header contract, exactly like `x-ratelimit-*`, and
///   it is unambiguously a credential problem that no amount of waiting fixes.
///   `ziee-ai` is an organization, so this is a real operator to cover.
///
/// For every OTHER `403` the only discriminator is the body's `message` prose,
/// which is not a contract — matching on it is the same brittleness as
/// validating a token's shape, which is explicitly wrong here (GitHub has
/// several valid token formats, adds more, and a shape check catches neither an
/// expired nor a revoked one). So the rule is conservative, and the asymmetry
/// justifies it: NOT falling back on an exotic `403` leaves the pre-fix
/// behaviour, which is no worse than today; falling back WRONGLY actively
/// misleads the operator and burns the 60/hr/IP budget.
///
/// Reading no body also leaves the `reqwest::Response` un-consumed, so the
/// callers' existing success/error paths are untouched.
fn is_auth_rejection(status: reqwest::StatusCode, headers: &reqwest::header::HeaderMap) -> bool {
    match status.as_u16() {
        401 => true,
        // The ONE `403` that carries its own documented header contract.
        // GitHub answers `403` + `X-GitHub-SSO: required; url=…` when a PAT is
        // valid but has not been authorized for a SAML-SSO-enforced org — and
        // `ziee-ai` is an org, so this is a real operator, not a hypothetical.
        // It is a credential problem with no waiting that fixes it, and the
        // signal is a HEADER, not body prose, so it is exactly as contractual
        // as `x-ratelimit-*`. Every other 403 stays unclassifiable and
        // therefore not a rejection.
        403 => headers.contains_key("x-github-sso"),
        _ => false,
    }
}

/// GitHub repo slug for an engine's fork.
fn engine_repo(engine: EngineType) -> &'static str {
    match engine {
        EngineType::Llamacpp => "ziee-ai/llama.cpp",
        EngineType::Mistralrs => "ziee-ai/mistral.rs",
    }
}

/// The binary name *inside* a release archive (`.exe` on Windows).
fn engine_binary_name(engine: EngineType, platform: &str) -> &'static str {
    match (engine, platform == "windows") {
        (EngineType::Llamacpp, false) => "llama-server",
        (EngineType::Llamacpp, true) => "llama-server.exe",
        (EngineType::Mistralrs, false) => "mistralrs-server",
        (EngineType::Mistralrs, true) => "mistralrs-server.exe",
    }
}

/// The archive-name stem (no `.exe`) used in release asset filenames.
fn archive_stem(engine: EngineType) -> &'static str {
    match engine {
        EngineType::Llamacpp => "llama-server",
        EngineType::Mistralrs => "mistralrs-server",
    }
}

/// Release archive extension for a platform (`zip` on Windows, else `tar.gz`).
fn archive_ext(platform: &str) -> &'static str {
    if platform == "windows" { "zip" } else { "tar.gz" }
}

/// The release asset filename for one (engine, platform, arch, backend):
/// `"{stem}-{platform}-{arch}-{backend}.{ext}"`. The single source of truth
/// for both the download URL and asset-readiness detection.
fn archive_name(engine: EngineType, platform: &str, arch: &str, backend: &str) -> String {
    format!(
        "{}-{}-{}-{}.{}",
        archive_stem(engine),
        platform,
        arch,
        backend,
        archive_ext(platform),
    )
}

/// If `asset` is the release archive for this (engine, platform, arch),
/// return its backend segment (e.g. `cpu`, `cuda`); else `None`.
///
/// Naturally rejects sibling `.sig` assets (`….tar.gz.sig` does not end in
/// `.tar.gz`) and other-arch/other-platform archives.
fn asset_backend(engine: EngineType, platform: &str, arch: &str, asset: &str) -> Option<String> {
    let prefix = format!("{}-{}-{}-", archive_stem(engine), platform, arch);
    let suffix = format!(".{}", archive_ext(platform));
    asset
        .strip_prefix(&prefix)?
        .strip_suffix(&suffix)
        .map(|s| s.to_string())
}

/// Platforms an engine release can publish for. Used to parse an asset name
/// back into its `(platform, arch, backend)` tuple.
const KNOWN_PLATFORMS: [&str; 3] = ["linux", "macos", "windows"];
/// Architectures an engine release can publish for.
const KNOWN_ARCHES: [&str; 2] = ["x86_64", "aarch64"];

/// Parse a release asset filename back into the `(platform, arch, backend)`
/// tuple `POST /versions/download` requires — the inverse of [`archive_name`].
///
/// [`asset_backend`] can only answer "is this asset for the host I already
/// know about"; discovery needs the opposite direction: given an arbitrary
/// asset, which installable combination IS it? Without this a discovery
/// response could only ever describe the host's own variants, leaving a caller
/// to guess `platform` and `arch` — the exact gap this branch exists to close.
///
/// Returns `None` for anything that is not an engine archive: sibling `.sig`
/// files, checksum sidecars, source tarballs, and any name whose platform or
/// arch segment is not one we publish (so a future/unknown token is skipped
/// rather than surfaced as a bogus installable variant).
pub fn parse_asset_variant(engine: EngineType, asset: &str) -> Option<(String, String, String)> {
    let stem = format!("{}-", archive_stem(engine));
    let rest = asset.strip_prefix(&stem)?;

    // Extension is platform-dependent, so try both and keep whichever matches.
    let rest = rest
        .strip_suffix(".tar.gz")
        .or_else(|| rest.strip_suffix(".zip"))?;

    // `{platform}-{arch}-{backend}`. `x86_64`/`aarch64` carry an underscore,
    // never a dash, so the first two dashes delimit exactly; the remainder is
    // the backend (which may itself contain dots, e.g. `cuda12.9`).
    let mut parts = rest.splitn(3, '-');
    let platform = parts.next()?;
    let arch = parts.next()?;
    let backend = parts.next()?;

    if !KNOWN_PLATFORMS.contains(&platform) || !KNOWN_ARCHES.contains(&arch) {
        return None;
    }
    if backend.is_empty() {
        return None;
    }
    // A windows archive is a .zip and everything else a .tar.gz; a mismatch
    // means the name is not one we produced, so don't advertise it.
    let expected_ext = archive_ext(platform);
    if !asset.ends_with(&format!(".{expected_ext}")) {
        return None;
    }

    Some((platform.to_string(), arch.to_string(), backend.to_string()))
}

/// One release asset, reduced to what update-checking needs:
/// the filename + GitHub's reported byte size (so the UI can render
/// the download size up-front and the user can make an informed
/// pick when CPU vs CUDA builds are very different).
#[derive(Debug, Clone)]
pub struct AssetInfo {
    pub name: String,
    pub size_bytes: u64,
}

/// Backends published for (engine, platform, arch) given a release's
/// assets. Empty ⇒ the release exists but its binary for this host
/// is not (yet) uploaded — the build-pending case.
pub fn available_backends(
    engine: EngineType,
    platform: &str,
    arch: &str,
    assets: &[AssetInfo],
) -> Vec<String> {
    assets
        .iter()
        .filter_map(|a| asset_backend(engine, platform, arch, &a.name))
        .collect()
}

/// The byte size of the host-matching binary archive for a specific
/// backend. Returns `None` when no asset matches (build-pending
/// case) or when GitHub omitted the `size` field (which it never
/// does in practice for published assets).
pub fn asset_size_for_backend(
    engine: EngineType,
    platform: &str,
    arch: &str,
    backend: &str,
    assets: &[AssetInfo],
) -> Option<u64> {
    let target = archive_name(engine, platform, arch, backend);
    assets.iter().find(|a| a.name == target).map(|a| a.size_bytes)
}

/// One upstream release, reduced to what update-checking needs.
#[derive(Debug, Clone)]
pub struct ReleaseInfo {
    /// Release tag (e.g. `v0.0.1-alpha`).
    pub version: String,
    /// GitHub draft flag — drafts are not public/installable.
    pub draft: bool,
    /// GitHub prerelease flag.
    pub prerelease: bool,
    /// ISO-8601 publish timestamp, if present.
    pub published_at: Option<String>,
    /// All assets attached to the release (filename + byte size).
    pub assets: Vec<AssetInfo>,
}

/// GitHub binary downloader
pub struct BinaryDownloader {
    binaries_dir: PathBuf,
    client: reqwest::Client,
}

/// Information about a downloaded binary
#[derive(Debug, Clone)]
pub struct BinaryInfo {
    /// Engine type
    pub engine: EngineType,

    /// Version tag (e.g., "v0.7.0")
    pub version: String,

    /// Platform (e.g., "linux", "macos", "windows")
    pub platform: String,

    /// Architecture (e.g., "x86_64", "aarch64")
    pub arch: String,

    /// Backend (e.g., "cpu", "cuda", "metal")
    pub backend: String,

    /// Local path to the binary
    pub path: PathBuf,

    /// File size in bytes
    // Recorded during download; not read by callers today.
    #[allow(dead_code)]
    pub size_bytes: u64,
}

impl BinaryDownloader {
    /// Create a new binary downloader with default cache directory
    pub fn new() -> Result<Self> {
        let binaries_dir = Self::default_binaries_dir()?;
        Self::with_binaries_dir(binaries_dir)
    }

    /// Create a downloader with custom binaries directory
    pub fn with_binaries_dir(binaries_dir: PathBuf) -> Result<Self> {
        std::fs::create_dir_all(&binaries_dir)?;

        let client = reqwest::Client::builder()
            .user_agent("llm-runtime/0.1.0")
            // Cap connection setup and per-read inactivity so a stalled peer
            // can't hang the data transfer forever. A blanket request timeout
            // is deliberately avoided — large engine downloads are legitimately
            // long-running; read_timeout only fires on no-progress.
            .connect_timeout(std::time::Duration::from_secs(30))
            .read_timeout(std::time::Duration::from_secs(60))
            .build()?;

        Ok(Self {
            binaries_dir,
            client,
        })
    }

    /// Get the default binaries directory
    /// Returns `~/.llm-runtime/binaries/`
    fn default_binaries_dir() -> Result<PathBuf> {
        let home = dirs::home_dir()
            .ok_or_else(|| RuntimeError::internal("Could not determine home directory"))?;

        Ok(home.join(".llm-runtime").join("binaries"))
    }

    /// Download a binary from GitHub releases with a per-chunk progress
    /// callback. The callback is invoked synchronously on every chunk
    /// read with `(bytes_received_so_far, total_bytes)`. `total_bytes`
    /// is `None` when the upstream omits Content-Length.
    ///
    /// # Arguments
    /// * `engine` - Engine type (Llamacpp or Mistralrs)
    /// * `version` - Version tag (e.g., "v0.7.0", use "latest" for latest release)
    /// * `platform` - Platform (e.g., "linux", "macos", "windows")
    /// * `arch` - Architecture (e.g., "x86_64", "aarch64")
    /// * `backend` - Backend (e.g., "cpu", "cuda", "metal")
    /// * `progress` - Progress callback (received_bytes, total_bytes)
    pub async fn download_with_progress<F>(
        &self,
        engine: EngineType,
        version: &str,
        platform: &str,
        arch: &str,
        backend: &str,
        progress: F,
    ) -> Result<BinaryInfo>
    where
        F: Fn(u64, Option<u64>) + Send + Sync,
    {
        // Determine repository and binary name (shared naming helpers, so
        // the download URL and asset-readiness detection never drift).
        let repo = engine_repo(engine);
        let binary_name = engine_binary_name(engine, platform);
        let archive_name = archive_name(engine, platform, arch, backend);

        // Resolve version if "latest"
        let resolved_version = if version == "latest" {
            self.get_latest_version(repo).await?
        } else {
            version.to_string()
        };

        tracing::info!(
            "Downloading {} {} for {}-{}-{}",
            match engine {
                EngineType::Llamacpp => "llama-server",
                EngineType::Mistralrs => "mistralrs-server",
            },
            resolved_version,
            platform,
            arch,
            backend
        );

        // Check if already cached
        let cache_dir = self.binaries_dir
            .join(match engine {
                EngineType::Llamacpp => "llamacpp",
                EngineType::Mistralrs => "mistralrs",
            })
            .join(&resolved_version)
            .join(format!("{}-{}-{}", platform, arch, backend));

        let binary_path = cache_dir.join(binary_name);

        if binary_path.exists() {
            tracing::info!("Binary already cached: {}", binary_path.display());
            let metadata = std::fs::metadata(&binary_path)?;

            // Ensure executable on Unix
            #[cfg(unix)]
            super::binary::ensure_executable(&binary_path)?;

            return Ok(BinaryInfo {
                engine,
                version: resolved_version,
                platform: platform.to_string(),
                arch: arch.to_string(),
                backend: backend.to_string(),
                path: binary_path,
                size_bytes: metadata.len(),
            });
        }

        // Construct GitHub release URL (host overridable in debug builds
        // via LLM_RUNTIME_RELEASE_MIRROR for integration tests).
        let download_url = format!(
            "{}/{}/releases/download/{}/{}",
            release_base_url(), repo, resolved_version, archive_name
        );

        tracing::info!("Downloading from: {}", download_url);

        // Create temporary download directory
        let temp_dir = self.binaries_dir.join(".tmp");
        std::fs::create_dir_all(&temp_dir)?;
        let temp_archive = temp_dir.join(&archive_name);

        // Download archive. A miss here is the automated-release race:
        // the tag can exist before CI finishes building + uploading the
        // per-platform binary, so a fetch that 404s means "build pending",
        // not "no such release". Surface that explicitly instead of a bare
        // HTTP error.
        self.download_file(&download_url, &temp_archive, Some(&progress))
            .await
            .map_err(|e| {
                RuntimeError::BinaryNotFound(format!(
                    "engine binary not published for {resolved_version} \
                     {platform}/{arch}/{backend} ({archive_name}): {e}. List the \
                     installable versions and their platform/arch/backend \
                     variants with GET /api/local-runtime/versions/available?engine={engine} \
                     and retry with a combination from that list. (If the release \
                     was just created, its CI build may still be in progress.)"
                ))
            })?;

        // Best-effort cosign-keyless artifact fetch. We pull the
        // sibling `.sig` when published and log the outcome, but the
        // install proceeds either way — the operator-facing
        // `allow_unsigned_downloads` gate has been removed (downloads
        // are always permitted; cryptographic verification will be
        // re-introduced once the fork CI signs releases). Operators
        // that need stricter handling pre-stage the binary
        // out-of-band.
        let sig_url = format!("{}.sig", download_url);
        let sig_path = temp_dir.join(format!("{}.sig", archive_name));
        // Sig fetch doesn't report progress — it's a tiny artifact and
        // the surrounding download has already left a 100% progress
        // frame in the SSE replay buffer.
        match self.download_file(&sig_url, &sig_path, None).await {
            Ok(()) => {
                tracing::info!(
                    "cosign sibling .sig downloaded for {} (verification not \
                     yet wired — install proceeds unconditionally)",
                    archive_name
                );
            }
            Err(e) => {
                tracing::warn!(
                    "cosign sibling .sig not available for {} ({e}); install \
                     proceeds unverified (TOFU) until the fork CI publishes \
                     signed releases",
                    archive_name
                );
            }
        }

        // Extract binary from archive
        std::fs::create_dir_all(&cache_dir)?;

        let extract_result = if platform == "windows" {
            self.extract_zip(&temp_archive, &cache_dir, binary_name)
        } else {
            self.extract_tar_gz(&temp_archive, &cache_dir, binary_name)
        };

        // Clean up the temporary archive AND the sibling `.sig` regardless of
        // whether extraction succeeded — otherwise a failed extract (or the
        // never-removed `.sig`) leaves orphaned files in the temp dir forever.
        let _ = std::fs::remove_file(&temp_archive);
        let _ = std::fs::remove_file(&sig_path);
        extract_result?;

        // Ensure executable on Unix
        #[cfg(unix)]
        super::binary::ensure_executable(&binary_path)?;

        let metadata = std::fs::metadata(&binary_path)?;

        tracing::info!("Binary downloaded: {}", binary_path.display());

        Ok(BinaryInfo {
            engine,
            version: resolved_version,
            platform: platform.to_string(),
            arch: arch.to_string(),
            backend: backend.to_string(),
            path: binary_path,
            size_bytes: metadata.len(),
        })
    }

    /// GET a GitHub API URL with exponential-backoff retry on transient
    /// failures (network/timeout errors, HTTP 5xx, and 429 rate-limit), plus a
    /// ONE-SHOT anonymous re-issue when GitHub rejects the configured
    /// credential. A single hiccup on the release-resolution path shouldn't
    /// fail the whole download/version-check; persistent failures still surface
    /// the last error.
    ///
    /// Returns the [`CredentialStatus`] ALONGSIDE the result rather than inside
    /// it, because the credential's health is known whether the read succeeded
    /// or failed — and a failed read is exactly when the operator most needs to
    /// be told their token was the problem.
    async fn github_get_with_retry(
        &self,
        url: &str,
    ) -> (Result<reqwest::Response>, CredentialStatus) {
        const MAX_ATTEMPTS: u32 = 3;

        // EXACTLY ONE anonymous re-issue per call, enforced by the flag below
        // rather than by a counter with a limit. A counter would be both dead
        // (the flag alone already stops the second fallback) and WRONG at any
        // value above 1: "retries remaining" and "the credential has not been
        // refused yet" are different propositions, and at a limit of 2 the loop
        // would re-present an already-rejected token — the opposite of the
        // intent, since a rejection is not transient and re-sending the same
        // credential is pure waste. One boolean cannot drift from its meaning.
        //
        // Worst case upstream requests per call: MAX_ATTEMPTS transient
        // attempts + 1 anonymous re-issue = 4. The re-issue is deliberately
        // outside the transient budget (a rejection is not transient), but it
        // cannot multiply it: `attempt` is unchanged across the fallback and
        // continues from where it was.

        // Read the env var ONCE per call, so a mid-call change cannot make the
        // authenticated and anonymous attempts disagree about what was tried.
        // Withheld entirely from an untrusted target (see
        // `credential_target_is_trusted`), so a misconfigured debug mirror
        // never receives an operator's real credential. The check is against
        // `url` — the string actually being requested — not a re-derived
        // `api_base_url()`, so the value guarded is the value used.
        //
        // A token that cannot become a header value is treated as unusable
        // rather than attached: reqwest defers that failure to `send()`, where
        // the loop would misfile it as a transient error, burn every attempt,
        // and NEVER reach the anonymous fallback — an invalid credential
        // failing worse than no credential, the exact inversion this fixes.
        let token = github_token()
            .filter(|_| credential_target_is_trusted(url))
            .filter(|t| reqwest::header::HeaderValue::from_str(&format!("Bearer {t}")).is_ok());
        // Starts at `Unverified` when a token exists, NOT `Used`: until GitHub
        // has actually answered, nothing is known about the credential, and
        // `Used` is a claim that upstream ACCEPTED it. Promoted on the first
        // real response; downgraded to `Rejected` if that response refuses it.
        let mut credential = if token.is_some() {
            CredentialStatus::Unverified
        } else {
            CredentialStatus::Absent
        };
        let mut credential_refused = false;
        let mut attempt: u32 = 1;

        loop {
            // Present the credential unless a previous response refused it.
            let authenticated = token.is_some() && !credential_refused;
            let mut req = self
                .client
                .get(url)
                .header("Accept", "application/vnd.github.v3+json")
                .timeout(std::time::Duration::from_secs(30));
            // Authenticate when the operator supplied a token. Unauthenticated
            // GitHub allows 60 requests/hour/IP, which a shared egress IP
            // exhausts quickly; a token lifts that to 5000/hour. This is the
            // ONLY place the token value is used — it is never logged, never
            // placed in an error string, never serialized onto a response.
            if let Some(token) = token.as_deref().filter(|_| authenticated) {
                req = req.header("Authorization", format!("Bearer {token}"));
            }
            let result = req.send().await;

            // An INVALID credential must not fail worse than no credential at
            // all. When GitHub rejects the token, re-issue the same request
            // once with no `Authorization` header — the path the operator would
            // have had with `GITHUB_TOKEN` unset, which for a public repo
            // ordinarily answers 200. This is a DIFFERENT request, not another
            // try of the same one, so it deliberately does not consume a
            // transient attempt.
            if authenticated
                && let Ok(resp) = &result
                && is_auth_rejection(resp.status(), resp.headers())
            {
                tracing::warn!(
                    url,
                    status = %resp.status(),
                    "GitHub rejected the configured GITHUB_TOKEN; retrying anonymously"
                );
                credential = CredentialStatus::Rejected;
                credential_refused = true;
                continue;
            }

            // GitHub proved it recognized this credential. Only NOW is `Used`
            // an observed fact rather than an assumption.
            //
            // The guard is NOT merely "a response arrived": a bare 403
            // (revoked / SAML-blocked / missing scope), a 404 (the repo is
            // invisible to this token) and a persistent 5xx are all
            // `Ok(Response)` and none is evidence of acceptance. Reporting
            // `used` for them would tell an operator with a dead token that it
            // is fine — the original defect with the blame inverted.
            //
            // A rejection recorded on an earlier attempt is never overwritten:
            // the anonymous re-issue succeeding does not make the token valid.
            // ONLY a SERVED request proves acceptance. An earlier draft also
            // counted a throttle (429, or 403 with an exhausted budget) as
            // proof, on the premise that GitHub must authenticate a request
            // before rate-limiting it. That premise is undocumented, and if it
            // is wrong — a shared egress IP whose ANONYMOUS budget is spent may
            // be throttled before the credential is judged at all — it reports
            // `used` for a dead token: the original defect with the blame
            // inverted. `Unverified` is never wrong, and the reason string
            // still names the 403/429, so being careful hides nothing.
            if authenticated
                && let Ok(resp) = &result
                && resp.status().is_success()
                && credential == CredentialStatus::Unverified
            {
                credential = CredentialStatus::Used;
            }

            let transient = match &result {
                Ok(resp) => resp.status().is_server_error() || resp.status().as_u16() == 429,
                Err(_) => true,
            };
            if transient && attempt < MAX_ATTEMPTS {
                let delay = std::time::Duration::from_millis(500 * 2u64.pow(attempt - 1));
                tracing::warn!(
                    "GitHub API {url}: transient failure, retrying in {delay:?} (attempt {attempt}/{MAX_ATTEMPTS})"
                );
                attempt += 1;
                tokio::time::sleep(delay).await;
                continue;
            }
            return (result.map_err(RuntimeError::from), credential);
        }
    }

    /// Get the latest release version from GitHub
    async fn get_latest_version(&self, repo: &str) -> Result<String> {
        let url = format!("{}/repos/{}/releases/latest", api_base_url(), repo);

        let (result, credential) = self.github_get_with_retry(&url).await;
        // Not `result?` — a transport failure on the anonymous re-issue would
        // drop the credential verdict on exactly the failure the operator most
        // needs explained.
        let response = match result {
            Ok(response) => response,
            Err(e) => {
                return Err(RuntimeError::network(format!(
                    "Failed to get latest release: {e}{}",
                    credential.failure_note()
                )));
            }
        };

        if !response.status().is_success() {
            return Err(RuntimeError::network(format!(
                "Failed to get latest release: HTTP {}{}",
                response.status(),
                credential.failure_note()
            )));
        }

        let json: serde_json::Value = response.json().await?;
        let tag_name = json["tag_name"]
            .as_str()
            .ok_or_else(|| RuntimeError::network("Could not parse latest release tag"))?;

        Ok(tag_name.to_string())
    }

    /// List an engine's upstream releases (newest first, as GitHub returns
    /// them), each reduced to a [`ReleaseInfo`]. Mirror-aware via
    /// [`api_base_url`] (so the integration suite can point it at the mock
    /// release server — same override the download path uses).
    pub async fn list_releases(
        &self,
        engine: EngineType,
    ) -> (Result<Vec<ReleaseInfo>>, CredentialStatus) {
        let url = format!("{}/repos/{}/releases", api_base_url(), engine_repo(engine));

        let (result, credential) = self.github_get_with_retry(&url).await;
        let parsed = Self::parse_release_list(result, credential).await;
        (parsed, credential)
    }

    /// Turn a raw release-list response into [`ReleaseInfo`]s. Split out of
    /// [`Self::list_releases`] purely so the credential status can be returned
    /// on BOTH the success and failure paths without an early `?` dropping it.
    async fn parse_release_list(
        result: Result<reqwest::Response>,
        credential: CredentialStatus,
    ) -> Result<Vec<ReleaseInfo>> {
        // Not `result?` — see the sibling note in `get_latest_version`.
        let response = match result {
            Ok(response) => response,
            Err(e) => {
                return Err(RuntimeError::network(format!(
                    "Failed to list releases: {e}{}",
                    credential.failure_note()
                )));
            }
        };

        if !response.status().is_success() {
            return Err(RuntimeError::network(format!(
                "Failed to list releases: HTTP {}{}",
                response.status(),
                credential.failure_note()
            )));
        }

        let releases: Vec<serde_json::Value> = response.json().await?;

        Ok(releases
            .iter()
            .filter_map(|r| {
                let version = r["tag_name"].as_str()?.to_string();
                // GitHub returns `assets[].size` as an integer (bytes).
                // We thread it through to the UI so the download row
                // can show "12.3 MB" before the user clicks Download.
                let assets = r["assets"]
                    .as_array()
                    .map(|assets| {
                        assets
                            .iter()
                            .filter_map(|a| {
                                let name = a["name"].as_str()?.to_string();
                                let size_bytes = a["size"].as_u64().unwrap_or(0);
                                Some(AssetInfo { name, size_bytes })
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                Some(ReleaseInfo {
                    version,
                    draft: r["draft"].as_bool().unwrap_or(false),
                    prerelease: r["prerelease"].as_bool().unwrap_or(false),
                    published_at: r["published_at"].as_str().map(String::from),
                    assets,
                })
            })
            .collect())
    }

    /// Download a file with progress bar.
    ///
    /// Closes 08-llm-local-runtime F-06 (Medium): enforces a 2 GiB
    /// hard cap on downloaded bytes (single engine binary ≤ ~300 MB
    /// in practice; leaves headroom for fat CUDA builds). The
    /// surrounding `client` already has its `timeout()` set in
    /// BinaryDownloader::new; this method adds the size cap as the
    /// remaining missing defense. Without it, an attacker-controlled
    /// upstream (e.g. a hijacked GitHub mirror) could redirect to a
    /// /dev/zero stream and fill the host disk.
    ///
    /// Note: this function does NOT cryptographically verify the
    /// downloaded binary. The right shape is a cosign-keyless verify
    /// (matches the `sigstore` crate already pulled by code_sandbox)
    /// against a `.sig` artifact published alongside each engine
    /// binary. That requires the fork release pipeline to actually
    /// sign (Actions OIDC + cosign sign-blob) — until that ships,
    /// this download path is TOFU. Operators reading the SBOM should
    /// confirm the upstream GitHub Releases page hashes match.
    async fn download_file(
        &self,
        url: &str,
        dest: &Path,
        progress: Option<&(dyn Fn(u64, Option<u64>) + Send + Sync)>,
    ) -> Result<()> {
        const MAX_DOWNLOAD_BYTES: u64 = 2 * 1024 * 1024 * 1024; // 2 GiB

        // Get file size from HEAD request
        let head_response = self.client.head(url).send().await?;

        if !head_response.status().is_success() {
            return Err(RuntimeError::network(format!(
                "Failed to access file: HTTP {}",
                head_response.status()
            )));
        }

        let total_size = head_response
            .headers()
            .get(reqwest::header::CONTENT_LENGTH)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(0);

        // Pre-check the Content-Length when present; fail fast before
        // streaming a single byte.
        if total_size > MAX_DOWNLOAD_BYTES {
            return Err(RuntimeError::network(format!(
                "Refusing to download {} bytes (cap {} bytes / 2 GiB)",
                total_size, MAX_DOWNLOAD_BYTES
            )));
        }

        // No terminal progress bar in the server context — download
        // progress is surfaced to the UI via SSE elsewhere.
        tracing::debug!(
            "Downloading {} ({} bytes)",
            dest.file_name().unwrap().to_string_lossy(),
            total_size
        );

        // Download file
        let mut response = self.client.get(url).send().await?;

        if !response.status().is_success() {
            return Err(RuntimeError::network(format!(
                "Failed to download: HTTP {}",
                response.status()
            )));
        }

        let mut file = File::create(dest)?;
        let mut received: u64 = 0;
        let total_for_cb = if total_size > 0 { Some(total_size) } else { None };
        // Initial 0% frame so subscribers see the bar render at start.
        if let Some(cb) = progress {
            cb(0, total_for_cb);
        }

        while let Some(chunk) = response.chunk().await? {
            received = received.saturating_add(chunk.len() as u64);
            if received > MAX_DOWNLOAD_BYTES {
                // Drop the partial download.
                let _ = std::fs::remove_file(dest);
                return Err(RuntimeError::network(format!(
                    "Download exceeded {} bytes / 2 GiB cap; aborted",
                    MAX_DOWNLOAD_BYTES
                )));
            }
            file.write_all(&chunk)?;
            if let Some(cb) = progress {
                cb(received, total_for_cb);
            }
        }

        tracing::debug!("Downloaded {}", dest.file_name().unwrap().to_string_lossy());

        Ok(())
    }

    /// Extract binary and all shared libraries from tar.gz archive
    fn extract_tar_gz(&self, archive: &Path, dest_dir: &Path, binary_name: &str) -> Result<()> {
        let tar_gz = File::open(archive)?;
        let tar = flate2::read::GzDecoder::new(tar_gz);
        let mut archive = tar::Archive::new(tar);

        // Enable preservation of permissions, ownership, and symlinks
        archive.set_preserve_permissions(true);
        archive.set_preserve_mtime(true);
        archive.set_unpack_xattrs(true);

        let mut binary_found = false;

        for entry in archive.entries()? {
            let mut entry = entry?;
            let entry_type = entry.header().entry_type();

            // Skip directories
            if entry_type.is_dir() {
                continue;
            }

            // Owned filename (no directory prefix). We FLATTEN every entry
            // into dest_dir, so any archive subdir structure is dropped.
            // Owning it ends the immutable borrow of `entry` before we
            // later need `&mut entry` for `unpack`.
            let file_name = entry
                .path()
                .ok()
                .and_then(|p| p.file_name().and_then(|n| n.to_str()).map(|s| s.to_string()))
                .unwrap_or_default();
            if file_name.is_empty() {
                continue;
            }

            // Hardlinks are still rejected: harder to validate safely and
            // not needed for the SONAME chains we care about.
            if entry_type.is_hard_link() {
                tracing::warn!("Skipping hardlink entry in archive: {}", file_name);
                continue;
            }

            let is_library = file_name.ends_with(".so")
                || file_name.contains(".so.")
                || file_name.ends_with(".dylib")
                || file_name.ends_with(".dll");

            // Symlinks: dynamically-linked engine releases ship SONAME
            // symlinks (`libfoo.so.0 -> libfoo.so.0.1.2`) that the loader
            // NEEDs at runtime — dropping them breaks the engine. We
            // RECREATE a symlink only when it's a library name AND its
            // target is a single, same-directory filename. Anything with
            // an absolute path, `..`, or multiple components is an escape
            // attempt and is rejected — preserving the F-05 path-traversal
            // guard (a `lib_evil.so -> /etc/passwd` link is never created).
            if entry_type.is_symlink() {
                let link: Option<PathBuf> =
                    entry.link_name().ok().flatten().map(|c| c.into_owned());
                match link.as_deref().and_then(safe_same_dir_symlink_target) {
                    Some(target) if is_library => {
                        let link_path = dest_dir.join(&file_name);
                        recreate_symlink(&target, &link_path)?;
                        tracing::debug!(
                            "Recreated library symlink: {} -> {}",
                            link_path.display(),
                            target.to_string_lossy()
                        );
                    }
                    _ => {
                        tracing::warn!(
                            "Skipping unsafe or non-library symlink entry in archive: {} -> {:?}",
                            file_name, link
                        );
                    }
                }
                continue;
            }

            // Extract the binary to the root of dest_dir.
            if file_name == binary_name {
                let dest_path = dest_dir.join(binary_name);
                entry.unpack(&dest_path)?;
                tracing::info!("Extracted binary: {}", dest_path.display());
                binary_found = true;
                continue;
            }

            // Extract shared libraries (.so / .dylib / .dll real files).
            if is_library {
                let dest_path = dest_dir.join(&file_name);
                entry.unpack(&dest_path)?;
                tracing::debug!("Extracted library: {}", dest_path.display());
            }
        }

        if !binary_found {
            return Err(RuntimeError::internal(format!(
                "Binary '{}' not found in archive",
                binary_name
            )));
        }

        Ok(())
    }

    /// Extract binary and all DLLs from zip archive
    fn extract_zip(&self, archive: &Path, dest_dir: &Path, binary_name: &str) -> Result<()> {
        let file = File::open(archive)?;
        let mut archive = zip::ZipArchive::new(file)?;

        let mut binary_found = false;

        for i in 0..archive.len() {
            let mut file = archive.by_index(i)?;
            let name = file.name();

            // Skip directories
            if name.ends_with('/') || name.ends_with('\\') {
                continue;
            }

            // Get just the filename without path
            let file_name = std::path::Path::new(name)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("");

            // Extract binary
            if file_name == binary_name {
                let dest_path = dest_dir.join(binary_name);
                let mut outfile = File::create(&dest_path)?;
                std::io::copy(&mut file, &mut outfile)?;
                tracing::info!("Extracted binary: {}", dest_path.display());
                binary_found = true;
                continue;
            }

            // Extract DLLs (Windows shared libraries)
            if file_name.ends_with(".dll") {
                let dest_path = dest_dir.join(file_name);
                let mut outfile = File::create(&dest_path)?;
                std::io::copy(&mut file, &mut outfile)?;
                tracing::debug!("Extracted DLL: {}", dest_path.display());
            }
        }

        if !binary_found {
            return Err(RuntimeError::internal(format!(
                "Binary '{}' not found in archive",
                binary_name
            )));
        }

        Ok(())
    }

    /// List all cached binaries
    pub fn list_binaries(&self) -> Result<Vec<BinaryInfo>> {
        let mut binaries = Vec::new();

        if !self.binaries_dir.exists() {
            return Ok(binaries);
        }

        // Iterate through engine directories
        for engine_entry in std::fs::read_dir(&self.binaries_dir)? {
            let engine_entry = engine_entry?;
            let engine_dir = engine_entry.path();

            if !engine_dir.is_dir() || engine_dir.file_name().unwrap() == ".tmp" {
                continue;
            }

            let engine = match engine_dir.file_name().unwrap().to_str() {
                Some("llamacpp") => EngineType::Llamacpp,
                Some("mistralrs") => EngineType::Mistralrs,
                _ => continue,
            };

            // Iterate through version directories
            for version_entry in std::fs::read_dir(&engine_dir)? {
                let version_entry = version_entry?;
                let version_dir = version_entry.path();

                if !version_dir.is_dir() {
                    continue;
                }

                let version = version_dir.file_name().unwrap().to_string_lossy().to_string();

                // Iterate through platform-arch-backend directories
                for build_entry in std::fs::read_dir(&version_dir)? {
                    let build_entry = build_entry?;
                    let build_dir = build_entry.path();

                    if !build_dir.is_dir() {
                        continue;
                    }

                    let build_name = build_dir.file_name().unwrap().to_string_lossy();
                    let parts: Vec<&str> = build_name.split('-').collect();

                    if parts.len() != 3 {
                        continue;
                    }

                    let (platform, arch, backend) = (parts[0], parts[1], parts[2]);

                    // Find binary file
                    let binary_name = match engine {
                        EngineType::Llamacpp => {
                            if platform == "windows" { "llama-server.exe" } else { "llama-server" }
                        },
                        EngineType::Mistralrs => {
                            if platform == "windows" { "mistralrs-server.exe" } else { "mistralrs-server" }
                        },
                    };

                    let binary_path = build_dir.join(binary_name);

                    if binary_path.exists() {
                        let metadata = std::fs::metadata(&binary_path)?;

                        binaries.push(BinaryInfo {
                            engine,
                            version: version.clone(),
                            platform: platform.to_string(),
                            arch: arch.to_string(),
                            backend: backend.to_string(),
                            path: binary_path,
                            size_bytes: metadata.len(),
                        });
                    }
                }
            }
        }

        Ok(binaries)
    }

}

/// Accept a symlink target only when it names a single entry in the SAME
/// directory (e.g. `libfoo.so.0 -> libfoo.so.0.1.2`). Returns the bare
/// target filename when safe; returns `None` for absolute targets, `..`,
/// or any multi-component path — those are escape attempts and the F-05
/// path-traversal guard rejects them (we flatten everything into one dir,
/// so a same-dir symlink is the only shape we can safely honor).
fn safe_same_dir_symlink_target(link: &Path) -> Option<std::ffi::OsString> {
    use std::path::Component;
    let mut comps = link.components();
    match (comps.next(), comps.next()) {
        (Some(Component::Normal(name)), None) => Some(name.to_os_string()),
        _ => None,
    }
}

/// Create a relative, same-dir symlink at `link_path` pointing to
/// `target`. Unix-only: on other platforms shared-library SONAME symlinks
/// don't apply (Windows ships DLLs as regular files), so this is a no-op.
fn recreate_symlink(target: &std::ffi::OsStr, link_path: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        // Remove any stale entry so re-extraction is idempotent.
        let _ = std::fs::remove_file(link_path);
        std::os::unix::fs::symlink(target, link_path)
            .map_err(|e| RuntimeError::internal(format!("symlink create failed: {e}")))?;
        Ok(())
    }
    #[cfg(not(unix))]
    {
        let _ = (target, link_path);
        Ok(())
    }
}

impl Default for BinaryDownloader {
    fn default() -> Self {
        Self::new().expect("Failed to create default binary downloader")
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    /// Serializes every test in this module that mutates process environment.
    /// `set_var`/`remove_var` are process-global and cargo runs tests in
    /// parallel threads, so two env-mutating tests must never overlap — this
    /// module now has more than one (mirror overrides + `GITHUB_TOKEN`).
    static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    /// Base hosts default to the real GitHub endpoints when the (debug-only)
    /// mirror env vars are unset, and — in debug builds — honor the override
    /// while trimming a trailing slash so URL construction never
    /// double-slashes. Kept as ONE test because it mutates process env.
    #[test]
    fn base_urls_default_to_github_and_honor_mirror_override() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        // SAFETY: edition-2024 marks env mutation unsafe (it's a process
        // global). ENV_LOCK serializes this against the other env-mutating
        // test in this module, so there's no concurrent writer to race with.
        unsafe {
            std::env::remove_var("LLM_RUNTIME_RELEASE_MIRROR");
            std::env::remove_var("LLM_RUNTIME_API_MIRROR");
        }
        assert_eq!(release_base_url(), "https://github.com");
        assert_eq!(api_base_url(), "https://api.github.com");

        // The override path only exists in debug builds.
        #[cfg(debug_assertions)]
        unsafe {
            std::env::set_var("LLM_RUNTIME_RELEASE_MIRROR", "http://127.0.0.1:9999/");
            assert_eq!(release_base_url(), "http://127.0.0.1:9999");
            // Empty is ignored — falls back to the default.
            std::env::set_var("LLM_RUNTIME_RELEASE_MIRROR", "");
            assert_eq!(release_base_url(), "https://github.com");
            std::env::remove_var("LLM_RUNTIME_RELEASE_MIRROR");
        }
    }

    #[test]
    fn safe_symlink_target_accepts_same_dir_rejects_escaping() {
        use std::path::Path;
        // Same-dir SONAME targets are accepted.
        assert_eq!(
            safe_same_dir_symlink_target(Path::new("libfoo.so.0.1.2")).as_deref(),
            Some(std::ffi::OsStr::new("libfoo.so.0.1.2"))
        );
        // Absolute, parent-escaping, and multi-component targets are rejected.
        assert!(safe_same_dir_symlink_target(Path::new("/etc/passwd")).is_none());
        assert!(safe_same_dir_symlink_target(Path::new("../../etc/passwd")).is_none());
        assert!(safe_same_dir_symlink_target(Path::new("sub/libfoo.so")).is_none());
        assert!(safe_same_dir_symlink_target(Path::new("..")).is_none());
    }

    /// A dynamically-linked engine release ships SONAME symlinks the loader
    /// needs (`libfoo.so.1 -> libfoo.so.1.2.3`). The extractor must keep
    /// those (recreated as same-dir relative symlinks) while still
    /// rejecting escaping symlinks (the F-05 guard).
    #[cfg(unix)]
    #[test]
    fn extract_tar_gz_keeps_safe_symlinks_and_rejects_escaping() {
        let tmp = tempfile::tempdir().unwrap();
        let archive_path = tmp.path().join("engine.tar.gz");

        {
            let f = File::create(&archive_path).unwrap();
            let enc = flate2::write::GzEncoder::new(f, flate2::Compression::fast());
            let mut b = tar::Builder::new(enc);

            let reg = |b: &mut tar::Builder<flate2::write::GzEncoder<File>>, name: &str, data: &[u8], mode: u32| {
                let mut h = tar::Header::new_gnu();
                h.set_size(data.len() as u64);
                h.set_mode(mode);
                h.set_cksum();
                b.append_data(&mut h, name, data).unwrap();
            };
            // Regular binary + a real versioned library.
            reg(&mut b, "llama-server", b"#!/bin/true\n", 0o755);
            reg(&mut b, "libfoo.so.1.2.3", b"ELF-ish-bytes", 0o644);

            let link = |b: &mut tar::Builder<flate2::write::GzEncoder<File>>, name: &str, target: &str| {
                let mut h = tar::Header::new_gnu();
                h.set_entry_type(tar::EntryType::Symlink);
                h.set_size(0);
                h.set_mode(0o777);
                b.append_link(&mut h, name, target).unwrap();
            };
            // SAFE same-dir SONAME symlink.
            link(&mut b, "libfoo.so.1", "libfoo.so.1.2.3");
            // ESCAPING symlinks (absolute + parent-traversal) — must be dropped.
            link(&mut b, "evil.so", "/etc/passwd");
            link(&mut b, "escape.so", "../../etc/passwd");

            b.into_inner().unwrap().finish().unwrap();
        }

        let dest = tmp.path().join("out");
        std::fs::create_dir_all(&dest).unwrap();
        let downloader = BinaryDownloader::with_binaries_dir(tmp.path().join("cache")).unwrap();
        downloader
            .extract_tar_gz(&archive_path, &dest, "llama-server")
            .unwrap();

        // Binary + real lib extracted.
        assert!(dest.join("llama-server").exists());
        assert!(dest.join("libfoo.so.1.2.3").exists());

        // Safe SONAME symlink recreated, relative, and resolves to the real file.
        let link_path = dest.join("libfoo.so.1");
        let meta = std::fs::symlink_metadata(&link_path).unwrap();
        assert!(meta.file_type().is_symlink(), "libfoo.so.1 must be a symlink");
        assert_eq!(
            std::fs::read_link(&link_path).unwrap(),
            std::path::PathBuf::from("libfoo.so.1.2.3")
        );
        assert!(std::fs::canonicalize(&link_path).unwrap().ends_with("libfoo.so.1.2.3"));

        // Escaping symlinks were rejected (never created).
        assert!(dest.join("evil.so").symlink_metadata().is_err(), "absolute-target symlink must be rejected");
        assert!(dest.join("escape.so").symlink_metadata().is_err(), "parent-traversal symlink must be rejected");
    }

    /// TEST-3a — every published variant of the REAL `ziee-ai/llama.cpp`
    /// v0.0.3-alpha asset set parses back into the `(platform, arch, backend)`
    /// tuple the download endpoint requires — including the non-host ones,
    /// which is the whole point (a caller must be able to pick a combination
    /// for a machine other than this server).
    #[test]
    fn parse_asset_variant_covers_every_published_asset() {
        let cases: &[(&str, (&str, &str, &str))] = &[
            ("llama-server-linux-x86_64-cpu.tar.gz", ("linux", "x86_64", "cpu")),
            ("llama-server-linux-x86_64-cuda12.9.tar.gz", ("linux", "x86_64", "cuda12.9")),
            ("llama-server-linux-x86_64-cuda13.2.tar.gz", ("linux", "x86_64", "cuda13.2")),
            ("llama-server-linux-x86_64-rocm5.7.tar.gz", ("linux", "x86_64", "rocm5.7")),
            ("llama-server-macos-aarch64-metal.tar.gz", ("macos", "aarch64", "metal")),
            ("llama-server-windows-x86_64-cpu.zip", ("windows", "x86_64", "cpu")),
            ("llama-server-windows-x86_64-cuda12.4.zip", ("windows", "x86_64", "cuda12.4")),
        ];
        for (asset, want) in cases {
            let got = parse_asset_variant(EngineType::Llamacpp, asset)
                .unwrap_or_else(|| panic!("failed to parse {asset}"));
            assert_eq!(
                (got.0.as_str(), got.1.as_str(), got.2.as_str()),
                *want,
                "wrong tuple for {asset}"
            );
        }

        // The mistral.rs fork uses a different binary stem.
        assert_eq!(
            parse_asset_variant(EngineType::Mistralrs, "mistralrs-server-linux-x86_64-cpu.tar.gz"),
            Some(("linux".into(), "x86_64".into(), "cpu".into()))
        );
    }

    /// TEST-3b — non-archive and malformed assets are rejected, so discovery
    /// never advertises a `.sig` sidecar or an unknown platform as something
    /// installable. Paired with the accept-cases above so a predicate that
    /// rejected everything could not pass.
    #[test]
    fn parse_asset_variant_rejects_non_archives() {
        let reject = [
            // sibling signature — must not become a "sig" backend
            "llama-server-linux-x86_64-cpu.tar.gz.sig",
            // checksum sidecar
            "llama-server-linux-x86_64-cpu.tar.gz.sha256",
            // other engine's stem
            "mistralrs-server-linux-x86_64-cpu.tar.gz",
            // unknown platform / arch tokens
            "llama-server-solaris-x86_64-cpu.tar.gz",
            "llama-server-linux-riscv64-cpu.tar.gz",
            // wrong extension for the platform (windows must be .zip)
            "llama-server-windows-x86_64-cpu.tar.gz",
            // missing backend segment
            "llama-server-linux-x86_64-.tar.gz",
            "llama-server-linux-x86_64.tar.gz",
            // unrelated release artifact
            "Source code (zip)",
        ];
        for asset in reject {
            assert_eq!(
                parse_asset_variant(EngineType::Llamacpp, asset),
                None,
                "{asset} must not parse as an installable variant"
            );
        }
    }

    /// TEST-7 — `GITHUB_TOKEN` handling. A set token is used; an unset or
    /// blank one yields `None` so no empty `Bearer` header is ever sent (GitHub
    /// answers 401 to that, which would break the working anonymous path).
    ///
    /// Serialized against the other env-var test via a mutex: `std::env::set_var`
    /// is process-global and Rust runs tests in threads.
    #[test]
    fn github_token_is_read_and_blank_is_treated_as_absent() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let prior = std::env::var("GITHUB_TOKEN").ok();

        unsafe { std::env::remove_var("GITHUB_TOKEN") };
        assert_eq!(github_token(), None, "unset must be None");

        unsafe { std::env::set_var("GITHUB_TOKEN", "") };
        assert_eq!(github_token(), None, "empty must be None, not Some(\"\")");

        unsafe { std::env::set_var("GITHUB_TOKEN", "   ") };
        assert_eq!(github_token(), None, "whitespace-only must be None");

        unsafe { std::env::set_var("GITHUB_TOKEN", " ghp_exampletoken ") };
        assert_eq!(
            github_token().as_deref(),
            Some("ghp_exampletoken"),
            "a set token must be returned, trimmed"
        );

        match prior {
            Some(v) => unsafe { std::env::set_var("GITHUB_TOKEN", v) },
            None => unsafe { std::env::remove_var("GITHUB_TOKEN") },
        }
    }

    // ── Credential rejection → anonymous fallback (the invalid-token defect) ──

    /// Build a `HeaderMap` from `(name, value)` pairs.
    fn headers(pairs: &[(&str, &str)]) -> reqwest::header::HeaderMap {
        let mut h = reqwest::header::HeaderMap::new();
        for (k, v) in pairs {
            h.insert(
                reqwest::header::HeaderName::from_bytes(k.as_bytes()).unwrap(),
                reqwest::header::HeaderValue::from_str(v).unwrap(),
            );
        }
        h
    }

    /// TEST-1 `[acceptance]` for INV-1 — a credential rejection and a refusal
    /// of an ACCEPTED credential are two different outcomes.
    ///
    /// `401` triggers the anonymous fallback; nothing else does. The `403`
    /// cases are the load-bearing half: a rate limit (primary OR secondary,
    /// with or without headers) must NOT be read as a rejection, because
    /// falling back would spend the scarce 60/hr/IP anonymous budget and tell
    /// the operator to replace a perfectly valid token. Widening the predicate
    /// to `401 | 403` — the intuitive implementation — turns this red.
    #[test]
    fn only_self_identifying_credential_refusals_trigger_the_fallback() {
        use reqwest::StatusCode;

        assert!(
            is_auth_rejection(StatusCode::UNAUTHORIZED, &headers(&[])),
            "401 is what GitHub answers for a bad/expired/revoked credential"
        );
        assert!(
            is_auth_rejection(
                StatusCode::FORBIDDEN,
                &headers(&[(
                    "x-github-sso",
                    "required; url=https://github.com/orgs/x/sso"
                )])
            ),
            "403 + X-GitHub-SSO is a documented header contract for a PAT that \
             is not authorized for a SAML-SSO org — a credential problem no \
             amount of waiting fixes"
        );

        // Every OTHER 403 is unclassifiable and must NOT fall back: the
        // anonymous bucket is a scarcer 60/hr/IP budget, and a wrong fallback
        // tells the operator to replace a perfectly valid token.
        for hdrs in [
            headers(&[]),
            headers(&[("x-ratelimit-remaining", "0")]),
            headers(&[("x-ratelimit-remaining", "4231")]),
            headers(&[("retry-after", "60")]),
        ] {
            assert!(
                !is_auth_rejection(StatusCode::FORBIDDEN, &hdrs),
                "a 403 that does not identify itself must not be read as a \
                 credential rejection"
            );
        }

        for status in [
            StatusCode::OK,
            StatusCode::NOT_FOUND,
            StatusCode::TOO_MANY_REQUESTS,
            StatusCode::INTERNAL_SERVER_ERROR,
            StatusCode::SERVICE_UNAVAILABLE,
        ] {
            assert!(
                !is_auth_rejection(status, &headers(&[("x-github-sso", "required")])),
                "{status} is not a credential verdict, whatever headers it carries"
            );
        }
    }

    /// TEST-2 `[acceptance]` for INV-3 — the fallback is driven by the
    /// RESPONSE, never by the token STRING.
    ///
    /// `github_token()` forwards every non-empty shape — `ghp_`, the newer
    /// `github_pat_`, an Actions `ghs_`, an OAuth `gho_`, and an entirely
    /// opaque value. A prefix check would have to reject at least one of them,
    /// and would still not catch an expired token, which is why validity is
    /// judged from GitHub's RESPONSE instead.
    ///
    /// The classifier half of INV-3 is not asserted here on purpose: since the
    /// round-2 change `is_auth_rejection` takes `(StatusCode, &HeaderMap)` and
    /// nothing else, so "its verdict does not depend on the token" is a
    /// property of the signature that no runtime loop can strengthen — varying
    /// the process credential around it would be theatre, and it would widen
    /// the `set_var` window for nothing.
    #[test]
    fn github_token_forwards_every_shape_and_filters_only_emptiness() {
        // Mutating GITHUB_TOKEN is process-global; ENV_LOCK serializes this
        // against the other env-mutating tests in this module, which would
        // otherwise observe (and permanently restore) each other's values.
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let prior = std::env::var("GITHUB_TOKEN").ok();

        for shape in [
            "ghp_0123456789abcdefghijklmnopqrstuvwxyz",
            "github_pat_11ABCDEFG0abcdefghijkl_ABCDEFGHIJKLMNOP",
            "ghs_actionsTokenLooksLikeThis0123456789",
            "gho_oauthTokenLooksLikeThis0123456789",
            "an-entirely-opaque-value",
        ] {
            // SAFETY: env mutation is process-global; ENV_LOCK is held.
            unsafe { std::env::set_var("GITHUB_TOKEN", shape) };
            assert_eq!(
                github_token().as_deref(),
                Some(shape),
                "every non-empty token shape must be forwarded — GitHub has \
                 several valid formats and adds more, so a shape check would \
                 reject valid credentials while still missing an expired one"
            );
        }

        // Padding is trimmed; emptiness remains the ONLY rejection rule.
        unsafe { std::env::set_var("GITHUB_TOKEN", "  ghp_padded  ") };
        assert_eq!(github_token().as_deref(), Some("ghp_padded"));
        unsafe { std::env::set_var("GITHUB_TOKEN", "   ") };
        assert_eq!(github_token(), None);

        match prior {
            Some(v) => unsafe { std::env::set_var("GITHUB_TOKEN", v) },
            None => unsafe { std::env::remove_var("GITHUB_TOKEN") },
        }
    }

    /// TEST-8 — the wire vocabulary is exactly `absent|used|unverified|rejected`.
    ///
    /// The vocabulary is duplicated in three places that cannot import each
    /// other (this enum, the OpenAPI doc comment, and the TS union), so the
    /// strings are pinned here. Only a rejection annotates a failing read, so
    /// an ordinary outage message is not padded with irrelevant advice — and
    /// the note names the VARIABLE, never a value.
    #[test]
    fn credential_status_wire_vocabulary_and_failure_note() {
        assert_eq!(CredentialStatus::Absent.as_str(), "absent");
        assert_eq!(CredentialStatus::Used.as_str(), "used");
        assert_eq!(CredentialStatus::Unverified.as_str(), "unverified");
        assert_eq!(CredentialStatus::Rejected.as_str(), "rejected");

        assert_eq!(CredentialStatus::Absent.failure_note(), "");
        assert_eq!(CredentialStatus::Used.failure_note(), "");
        assert_eq!(CredentialStatus::Unverified.failure_note(), "");
        let note = CredentialStatus::Rejected.failure_note();
        assert!(
            note.contains("GITHUB_TOKEN"),
            "the note must name the variable so the operator knows what to fix"
        );
        assert!(
            !note.contains("Bearer") && !note.contains("ghp_"),
            "and it must never carry, or hint at, a credential VALUE"
        );
    }

    /// The credential is withheld from any host that is not real GitHub (or, in
    /// a debug build, a loopback mirror). Without this, a dev/CI process
    /// holding a REAL token plus a misconfigured `LLM_RUNTIME_API_MIRROR` would
    /// transmit that credential to an arbitrary host in cleartext.
    #[test]
    fn credential_is_withheld_from_untrusted_targets() {
        for github in [
            "https://api.github.com",
            "https://api.github.com/repos/ziee-ai/llama.cpp/releases",
            "https://API.GITHUB.COM/repos/x/y/releases",
            "https://api.github.com./repos/x/y/releases",
        ] {
            assert!(
                credential_target_is_trusted(github),
                "{github} is the real GitHub API and must be authenticated"
            );
        }

        for hostile in [
            // Look-alike hosts.
            "https://api.github.com.evil.example",
            "http://evil.example",
            "https://evil.example/api.github.com",
            "http://10.0.0.5:8080",
            "http://[fd00::1]:9000",
            // Hosts that CONTAIN a loopback token — these are what a
            // `contains()` implementation would wrongly trust.
            "http://127.0.0.1.evil.example",
            "http://evil.localhost.example",
            "http://localhost.evil.com",
            // USERINFO smuggling: the real host is `evil.example`. A
            // split/rsplit authority parser reads the host as `localhost` and
            // the port as `8080@evil.example`, and ships the token offsite.
            "http://localhost:8080@evil.example",
            "http://127.0.0.1:9@evil.example/x",
            "https://api.github.com@evil.example",
            // Plain-http GitHub is not GitHub.
            "http://api.github.com",
            // Not a URL at all.
            "api.github.com",
            "",
        ] {
            assert!(
                !credential_target_is_trusted(hostile),
                "{hostile} must never receive the operator's GitHub credential"
            );
        }

        // The debug-only loopback seam the integration suite depends on.
        #[cfg(debug_assertions)]
        for loopback in [
            "http://127.0.0.1:41234",
            "http://localhost:41234",
            "http://[::1]:41234",
            // Port-less and path-bearing forms must work too — the earlier
            // string-surgery version mis-parsed a bracketed IPv6 with no port.
            "http://[::1]",
            "http://127.0.0.1",
            "http://127.0.0.1:41234/repos/x/y/releases",
            // Case is not significant in a host.
            "http://LOCALHOST:41234",
        ] {
            assert!(
                credential_target_is_trusted(loopback),
                "{loopback} is the test mock and must still exercise the \
                 authenticated path"
            );
        }
    }
}
