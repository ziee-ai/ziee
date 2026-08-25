// Pre-download model-file discovery.
//
// Lists the files in a Hugging Face or GitHub repo path so the UI can show
// what's available BEFORE a download, and classifies them with the shared
// mistral.rs-parity detector (`model_files`). Detection from the repo's git
// tree mirrors how the actual download (git clone) + the engine load work.

use aide::transform::TransformOperation;
use axum::{Json, debug_handler, extract::Query, http::StatusCode};
use reqwest::header::{ACCEPT, AUTHORIZATION, LINK, USER_AGENT};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use uuid::Uuid;

use crate::{
    common::{ApiResult, AppError},
    core::repository::Repos,
    modules::llm_model::model_files::{
        FileRole, ModelShape, classify, detect_weight_set, file_format_for, is_gguf,
    },
    modules::llm_repository::models::LlmRepository,
    modules::permissions::{RequirePermissions, with_permission},
    utils::url_validator::{OutboundUrlPolicy, build_validated_client, validate_outbound_url},
};

use super::super::permissions::LlmModelsCreate;

const UA: &str = concat!("ziee/", env!("CARGO_PKG_VERSION"));
const HTTP_TIMEOUT: Duration = Duration::from_secs(20);
/// Hard ceiling on the number of files RETAINED across pagination (bounds
/// the result vector). Each page's JSON body is still buffered by reqwest,
/// but the per-response size is additionally capped by `MAX_BODY_BYTES`.
const MAX_LISTED: usize = 50_000;
/// Reject an upstream listing response that declares more than this many
/// bytes (defense-in-depth against a hostile/compromised upstream).
const MAX_BODY_BYTES: u64 = 64 * 1024 * 1024;

// =====================================================
// Request / response types
// =====================================================

#[derive(Debug, Deserialize, JsonSchema)]
pub struct RepositoryFilesQuery {
    pub repository_id: Uuid,
    /// Repo path within the source, e.g. "meta-llama/Llama-3.1-8B-Instruct".
    pub path: String,
    /// Branch / revision (defaults to "main").
    pub branch: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RepositorySource {
    // Wire token pinned to "huggingface" (snake_case of HuggingFace would be
    // "hugging_face", which would break the generated client contract).
    #[serde(rename = "huggingface")]
    HuggingFace,
    Github,
    Unknown,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct RepositoryFile {
    pub path: String,
    pub size_bytes: i64,
    pub file_role: FileRole,
    /// "safetensors" | "pytorch" | "gguf" for weight files, else null.
    pub file_format: Option<String>,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct RepositoryFileListResponse {
    pub source: RepositorySource,
    pub shape: ModelShape,
    pub files: Vec<RepositoryFile>,
    /// A sensible default to pre-fill the download form's main filename.
    pub suggested_main_filename: Option<String>,
    /// True when the upstream listing was capped (very large GitHub trees).
    pub truncated: bool,
}

// =====================================================
// Handler
// =====================================================

/// GET /api/llm-models/repository-files
#[debug_handler]
pub async fn list_repository_files(
    _auth: RequirePermissions<(LlmModelsCreate,)>,
    Query(params): Query<RepositoryFilesQuery>,
) -> ApiResult<Json<RepositoryFileListResponse>> {
    let repo = Repos
        .llm_repository
        .get_by_id(params.repository_id)
        .await
        .map_err(|e| AppError::internal_with_id(e).to_api_error())?
        .ok_or_else(|| AppError::not_found("Repository").to_api_error())?;

    let path = params.path.trim().trim_matches('/').to_string();
    let branch = params
        .branch
        .as_deref()
        .map(str::trim)
        .filter(|b| !b.is_empty())
        .unwrap_or("main")
        .to_string();

    // The path and branch are interpolated into the upstream URL, so reject
    // anything that could smuggle a query/fragment/extra path segment.
    if !valid_repo_segment(&path) {
        return Err(
            AppError::bad_request("VALIDATION_ERROR", "Invalid repository path").to_api_error(),
        );
    }
    if !valid_repo_segment(&branch) {
        return Err(AppError::bad_request("VALIDATION_ERROR", "Invalid branch").to_api_error());
    }

    // Detect the source by the repository's actual host (exact / subdomain
    // suffix), NOT a substring of the URL — so a repo whose real host merely
    // contains "huggingface.co" (e.g. https://evil.example/huggingface.co)
    // can't route its stored bearer token at the canonical API.
    let host = url::Url::parse(&repo.url)
        .ok()
        .and_then(|u| u.host_str().map(|h| h.to_lowercase()))
        .unwrap_or_default();
    let host_is = |suffix: &str| host == suffix || host.ends_with(&format!(".{suffix}"));

    let (source, mut listed) = if host_is("huggingface.co") {
        (
            RepositorySource::HuggingFace,
            fetch_hf_files(&repo, &path, &branch).await?,
        )
    } else if host_is("github.com") {
        (
            RepositorySource::Github,
            fetch_github_files(&repo, &path, &branch).await?,
        )
    } else {
        // Auto-detect only supports HF/GitHub; UI keeps the manual filename.
        (RepositorySource::Unknown, ListedFiles::default())
    };

    // The download path copies from a NON-recursive listing of the cloned
    // repo, so weights nested in subdirectories can't actually be fetched.
    // Only surface top-level files so the picker never offers a file the
    // downloader can't retrieve. (Full nested-repo support would require a
    // recursive clone-copy in handlers::uploads.)
    top_level_only(&mut listed.files);

    let names: Vec<String> = listed.files.iter().map(|(p, _)| p.clone()).collect();
    let det = detect_weight_set(&names);

    let files: Vec<RepositoryFile> = listed
        .files
        .iter()
        .map(|(p, size)| RepositoryFile {
            path: p.clone(),
            size_bytes: *size,
            file_role: classify(p),
            file_format: file_format_for(p).map(str::to_string),
        })
        .collect();

    // For GGUF, refine the suggested quant with size awareness (prefer a
    // Q4_K_M, else the smallest gguf so the default download is small).
    let suggested_main_filename = if det.shape == ModelShape::Gguf {
        suggest_gguf_with_sizes(&listed.files).or(det.suggested_main)
    } else {
        det.suggested_main
    };

    Ok((
        StatusCode::OK,
        Json(RepositoryFileListResponse {
            source,
            shape: det.shape,
            files,
            suggested_main_filename,
            truncated: listed.truncated,
        }),
    ))
}

pub fn list_repository_files_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(LlmModelsCreate,)>(op)
        .id("LlmModel.listRepositoryFiles")
        .tag("LLM Models")
        .summary("List model files in a repository")
        .description(
            "Detect the model files available at a Hugging Face or GitHub repo path before \
             downloading, classified the same way the engine loads them.",
        )
        .response::<200, Json<RepositoryFileListResponse>>()
        .response_with::<400, (), _>(|res| res.description("Invalid repository path or branch"))
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
        .response_with::<403, (), _>(|res| {
            res.description("Upstream rejected the request (auth required or rate-limited)")
        })
        .response_with::<404, (), _>(|res| res.description("Repository or path not found"))
}

// =====================================================
// Upstream fetch
// =====================================================

#[derive(Default)]
struct ListedFiles {
    /// (path, size_bytes)
    files: Vec<(String, i64)>,
    truncated: bool,
}

type ApiErr = (StatusCode, AppError);

/// Keep only top-level files. The non-recursive clone-copy downloader can't
/// fetch weights nested in subdirectories, so the picker must not offer them.
fn top_level_only(files: &mut Vec<(String, i64)>) {
    files.retain(|(p, _)| !p.contains('/'));
}

/// A path / branch value that is safe to interpolate into the upstream URL.
/// Positive allowlist matching real HF/GitHub repo + ref grammar — rejects
/// anything that could change the host/path/query (`@`, `:`, `?`, `#`, space,
/// `\`, control chars, `..`).
fn valid_repo_segment(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 256
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-' | '/'))
        && !s.split('/').any(|seg| seg.is_empty() || seg == "..")
}

/// Outbound policy for upstream requests: allow loopback/private in debug
/// (so the LLM_MODEL_*_API_BASE test overrides work), public-only in release.
fn outbound_policy() -> OutboundUrlPolicy {
    if cfg!(debug_assertions) {
        OutboundUrlPolicy::DEV_LOCAL
    } else {
        OutboundUrlPolicy::PUBLIC_HTTP_OR_HTTPS
    }
}

/// SSRF-validated client: re-checks the scheme + IP of every redirect hop
/// (matching git/service.rs + oauth2.rs), so a 302 to 169.254.169.254 / a
/// loopback / an RFC1918 address can't be followed in release.
fn http_client() -> Result<reqwest::Client, ApiErr> {
    build_validated_client(outbound_policy()).map_err(|e| AppError::internal_with_id(e).to_api_error())
}

/// Pre-flight the URL itself (initial + each pagination hop) so a poisoned
/// base / cursor pointing at a blocked address is screened before `.send()`.
fn preflight(url: &str) -> Result<(), ApiErr> {
    validate_outbound_url(url, &outbound_policy())
        .map(|_| ())
        .map_err(|e| {
            AppError::bad_request("UPSTREAM_URL_BLOCKED", format!("Blocked upstream URL: {e}"))
                .to_api_error()
        })
}

/// Token to send as `Authorization: Bearer` for HF/GitHub (both use Bearer).
fn repo_bearer(repo: &LlmRepository) -> Option<String> {
    let t = match repo.auth_type.as_str() {
        "api_key" => repo.auth_config.api_key.clone(),
        "bearer_token" => repo.auth_config.token.clone(),
        _ => None,
    };
    t.filter(|t| !t.trim().is_empty())
}

fn hf_api_base() -> String {
    #[cfg(debug_assertions)]
    if let Ok(v) = std::env::var("LLM_MODEL_HF_API_BASE") {
        if !v.trim().is_empty() {
            return v;
        }
    }
    "https://huggingface.co".to_string()
}

fn github_api_base() -> String {
    #[cfg(debug_assertions)]
    if let Ok(v) = std::env::var("LLM_MODEL_GITHUB_API_BASE") {
        if !v.trim().is_empty() {
            return v;
        }
    }
    "https://api.github.com".to_string()
}

/// Map an upstream HTTP status to an ApiError for the listing call.
fn upstream_status_error(source: &str, status: reqwest::StatusCode) -> ApiErr {
    match status.as_u16() {
        404 => AppError::not_found(&format!("Model path on {source}")).to_api_error(),
        401 | 403 | 429 => AppError::forbidden(
            "UPSTREAM_REJECTED",
            format!("{source} rejected the request (auth required or rate-limited)"),
        )
        .to_api_error(),
        _ => AppError::internal_error(format!("{source} returned HTTP {status}")).to_api_error(),
    }
}

#[derive(Deserialize)]
struct HfTreeEntry {
    #[serde(rename = "type")]
    kind: String,
    path: String,
    #[serde(default)]
    size: i64,
    #[serde(default)]
    lfs: Option<HfLfs>,
}

#[derive(Deserialize)]
struct HfLfs {
    #[serde(default)]
    size: i64,
}

/// Real file size for an HF tree entry: the LFS object size when present
/// (the top-level `size` is the pointer size for LFS files), else `size`.
fn hf_entry_size(e: &HfTreeEntry) -> i64 {
    e.lfs
        .as_ref()
        .map(|l| l.size)
        .filter(|s| *s > 0)
        .unwrap_or(e.size)
}

/// Same host (exact or subdomain) as the pinned base, if both parse.
fn same_host(url: &str, pinned: &Option<String>) -> bool {
    match (url::Url::parse(url).ok().and_then(|u| u.host_str().map(str::to_string)), pinned) {
        (Some(h), Some(p)) => h.eq_ignore_ascii_case(p),
        _ => false,
    }
}

async fn fetch_hf_files(
    repo: &LlmRepository,
    path: &str,
    branch: &str,
) -> Result<ListedFiles, ApiErr> {
    let client = http_client()?;
    let bearer = repo_bearer(repo);

    let mut out = ListedFiles::default();
    // Non-recursive: we only surface top-level files anyway, so don't pull
    // (and then discard) the whole nested tree.
    let first = format!("{}/api/models/{}/tree/{}", hf_api_base(), path, branch);
    // Pin the first URL's host: the bearer token must not be forwarded to a
    // different host smuggled in via a poisoned Link "next" cursor.
    let pinned_host = url::Url::parse(&first)
        .ok()
        .and_then(|u| u.host_str().map(str::to_string));
    let mut next = Some(first);
    let mut pages = 0;

    while let Some(url) = next.take() {
        pages += 1;
        if pages > 50 {
            out.truncated = true;
            break;
        }
        preflight(&url)?;
        let mut req = client.get(&url).timeout(HTTP_TIMEOUT).header(USER_AGENT, UA);
        if let Some(ref t) = bearer {
            req = req.header(AUTHORIZATION, format!("Bearer {t}"));
        }
        let resp = req
            .send()
            .await
            .map_err(|e| AppError::internal_with_id(e).to_api_error())?;
        if !resp.status().is_success() {
            return Err(upstream_status_error("Hugging Face", resp.status()));
        }
        if let Some(len) = resp.content_length()
            && len > MAX_BODY_BYTES
        {
            return Err(AppError::forbidden(
                "UPSTREAM_TOO_LARGE",
                "Upstream listing response is too large",
            )
            .to_api_error());
        }

        // Cursor pagination via the Link header (rel="next") — only followed
        // when it stays on the pinned host.
        next = resp
            .headers()
            .get(LINK)
            .and_then(|v| v.to_str().ok())
            .and_then(parse_link_next)
            .filter(|u| same_host(u, &pinned_host));

        let entries: Vec<HfTreeEntry> = resp
            .json()
            .await
            .map_err(|e| AppError::internal_with_id(e).to_api_error())?;
        for e in entries {
            if e.kind == "file" {
                let size = hf_entry_size(&e);
                out.files.push((e.path, size));
            }
        }
        // Bound total memory across pages (defense-in-depth).
        if out.files.len() > MAX_LISTED {
            out.files.truncate(MAX_LISTED);
            out.truncated = true;
            break;
        }
    }

    Ok(out)
}

#[derive(Deserialize)]
struct GhTree {
    #[serde(default)]
    tree: Vec<GhEntry>,
    #[serde(default)]
    truncated: bool,
}

#[derive(Deserialize)]
struct GhEntry {
    path: String,
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    size: i64,
}

async fn fetch_github_files(
    repo: &LlmRepository,
    path: &str,
    branch: &str,
) -> Result<ListedFiles, ApiErr> {
    let client = http_client()?;
    let bearer = repo_bearer(repo);

    // GitHub's repo/trees API needs exactly owner/repo.
    if path.split('/').count() != 2 {
        return Err(AppError::bad_request(
            "VALIDATION_ERROR",
            "GitHub repository path must be 'owner/repo'",
        )
        .to_api_error());
    }

    let mut resp = gh_trees(&client, path, branch, &bearer).await?;
    // The default branch may be "master" (not the UI's "main" default): on a
    // 404, resolve the repo's real default branch and retry once.
    if resp.status().as_u16() == 404
        && let Some(default_branch) = gh_default_branch(&client, path, &bearer).await?
        && default_branch != branch
    {
        resp = gh_trees(&client, path, &default_branch, &bearer).await?;
    }
    if !resp.status().is_success() {
        return Err(upstream_status_error("GitHub", resp.status()));
    }
    if let Some(len) = resp.content_length()
        && len > MAX_BODY_BYTES
    {
        return Err(
            AppError::forbidden("UPSTREAM_TOO_LARGE", "Upstream listing response is too large")
                .to_api_error(),
        );
    }
    let tree: GhTree = resp
        .json()
        .await
        .map_err(|e| AppError::internal_with_id(e).to_api_error())?;

    let mut out = ListedFiles {
        truncated: tree.truncated,
        ..Default::default()
    };
    for e in tree.tree {
        if e.kind == "blob" {
            out.files.push((e.path, e.size));
            if out.files.len() > MAX_LISTED {
                out.files.truncate(MAX_LISTED);
                out.truncated = true;
                break;
            }
        }
    }
    Ok(out)
}

/// GET the GitHub git-trees listing for a branch/tree-ish (preflighted).
async fn gh_trees(
    client: &reqwest::Client,
    path: &str,
    branch: &str,
    bearer: &Option<String>,
) -> Result<reqwest::Response, ApiErr> {
    let url = format!("{}/repos/{}/git/trees/{}", github_api_base(), path, branch);
    preflight(&url)?;
    let mut req = client
        .get(&url)
        .timeout(HTTP_TIMEOUT)
        .header(ACCEPT, "application/vnd.github+json")
        .header(USER_AGENT, UA);
    if let Some(t) = bearer {
        req = req.header(AUTHORIZATION, format!("Bearer {t}"));
    }
    req.send()
        .await
        .map_err(|e| AppError::internal_with_id(e).to_api_error())
}

/// Resolve a GitHub repo's default branch (None if the repo isn't found).
async fn gh_default_branch(
    client: &reqwest::Client,
    path: &str,
    bearer: &Option<String>,
) -> Result<Option<String>, ApiErr> {
    let url = format!("{}/repos/{}", github_api_base(), path);
    preflight(&url)?;
    let mut req = client
        .get(&url)
        .timeout(HTTP_TIMEOUT)
        .header(ACCEPT, "application/vnd.github+json")
        .header(USER_AGENT, UA);
    if let Some(t) = bearer {
        req = req.header(AUTHORIZATION, format!("Bearer {t}"));
    }
    let resp = req
        .send()
        .await
        .map_err(|e| AppError::internal_with_id(e).to_api_error())?;
    if !resp.status().is_success() {
        return Ok(None);
    }
    #[derive(Deserialize)]
    struct GhRepo {
        default_branch: Option<String>,
    }
    let r: GhRepo = resp
        .json()
        .await
        .map_err(|e| AppError::internal_with_id(e).to_api_error())?;
    // Re-validate before interpolating the resolved branch into the next URL.
    Ok(r.default_branch.filter(|b| valid_repo_segment(b)))
}

/// Parse `<https://...&cursor=...>; rel="next"` → the next URL.
fn parse_link_next(link: &str) -> Option<String> {
    for part in link.split(',') {
        if part.contains("rel=\"next\"") {
            let start = part.find('<')?;
            let end = part[start + 1..].find('>')? + start + 1;
            return Some(part[start + 1..end].to_string());
        }
    }
    None
}

/// Among gguf files, prefer the smallest Q4_K_M, else the smallest gguf.
fn suggest_gguf_with_sizes(files: &[(String, i64)]) -> Option<String> {
    use crate::modules::llm_model::model_files::{basename, shard_prefix};

    let ggufs: Vec<&(String, i64)> = files.iter().filter(|(p, _)| is_gguf(p)).collect();
    if ggufs.is_empty() {
        return None;
    }
    let pick = ggufs
        .iter()
        .filter(|(p, _)| p.to_lowercase().contains("q4_k_m"))
        .min_by_key(|(_, s)| *s)
        .or_else(|| ggufs.iter().min_by_key(|(_, s)| *s))?;

    // If the smallest-by-size pick is a shard, normalize to its FIRST shard so
    // the suggestion matches the picker's collapsed first-shard option.
    if let Some(prefix) = shard_prefix(&pick.0) {
        if let Some((first, _)) = ggufs
            .iter()
            .filter(|(p, _)| shard_prefix(p).as_deref() == Some(prefix.as_str()))
            .min_by_key(|(p, _)| p.to_lowercase())
        {
            return Some(basename(first).to_string());
        }
    }
    Some(basename(&pick.0).to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn link_next_parsing() {
        let h = "<https://huggingface.co/api/models/x/tree/main?recursive=true&cursor=abc>; rel=\"next\"";
        assert_eq!(
            parse_link_next(h).as_deref(),
            Some("https://huggingface.co/api/models/x/tree/main?recursive=true&cursor=abc")
        );
        assert_eq!(parse_link_next("<https://x>; rel=\"prev\""), None);
    }

    #[test]
    fn hf_size_prefers_lfs() {
        let lfs = HfTreeEntry {
            kind: "file".into(),
            path: "x".into(),
            size: 134,
            lfs: Some(HfLfs { size: 4096 }),
        };
        assert_eq!(hf_entry_size(&lfs), 4096);
        let plain = HfTreeEntry { kind: "file".into(), path: "y".into(), size: 2048, lfs: None };
        assert_eq!(hf_entry_size(&plain), 2048);
        // LFS object reporting size 0 falls back to the top-level size.
        let zero = HfTreeEntry {
            kind: "file".into(),
            path: "z".into(),
            size: 500,
            lfs: Some(HfLfs { size: 0 }),
        };
        assert_eq!(hf_entry_size(&zero), 500);
    }

    #[test]
    fn cursor_host_pinning() {
        let pinned = Some("huggingface.co".to_string());
        assert!(same_host(
            "https://huggingface.co/api/models/x/tree/main?cursor=z",
            &pinned
        ));
        assert!(!same_host("https://evil.example/x", &pinned));
        assert!(!same_host("not a url", &pinned));
    }

    #[test]
    fn top_level_only_drops_nested() {
        let mut files = vec![
            ("onnx/model.onnx".to_string(), 1),
            ("sub/model.safetensors".to_string(), 2),
            ("model.safetensors".to_string(), 3),
            ("config.json".to_string(), 4),
        ];
        top_level_only(&mut files);
        assert_eq!(
            files.iter().map(|(p, _)| p.as_str()).collect::<Vec<_>>(),
            vec!["model.safetensors", "config.json"]
        );
    }

    #[test]
    fn gguf_suggestion_prefers_smallest_q4km() {
        let files = vec![
            ("a-Q8_0.gguf".to_string(), 100),
            ("a-Q4_K_M.gguf".to_string(), 40),
            ("a-Q5_K_M.gguf".to_string(), 60),
        ];
        assert_eq!(suggest_gguf_with_sizes(&files).as_deref(), Some("a-Q4_K_M.gguf"));
    }

    #[test]
    fn gguf_suggestion_falls_back_to_smallest() {
        let files = vec![
            ("a-Q8_0.gguf".to_string(), 100),
            ("a-Q5_K_M.gguf".to_string(), 60),
        ];
        assert_eq!(suggest_gguf_with_sizes(&files).as_deref(), Some("a-Q5_K_M.gguf"));
    }

    #[test]
    fn gguf_suggestion_normalizes_sharded_pick_to_first_shard() {
        // Shard 2 is smallest, but the suggestion must be the first shard so
        // it matches the picker's collapsed option.
        let files = vec![
            ("m-00001-of-00003.gguf".to_string(), 90),
            ("m-00002-of-00003.gguf".to_string(), 30),
            ("m-00003-of-00003.gguf".to_string(), 90),
        ];
        assert_eq!(
            suggest_gguf_with_sizes(&files).as_deref(),
            Some("m-00001-of-00003.gguf")
        );
    }

    #[test]
    fn source_serde_wire_tokens() {
        // Pin the generated-client contract (types.ts: 'huggingface' | ...).
        assert_eq!(
            serde_json::to_value(RepositorySource::HuggingFace).unwrap(),
            serde_json::json!("huggingface")
        );
        assert_eq!(
            serde_json::to_value(RepositorySource::Github).unwrap(),
            serde_json::json!("github")
        );
        assert_eq!(
            serde_json::to_value(RepositorySource::Unknown).unwrap(),
            serde_json::json!("unknown")
        );
    }

    #[test]
    fn rejects_unsafe_segments() {
        assert!(valid_repo_segment("meta-llama/Llama-3.1-8B-Instruct"));
        assert!(valid_repo_segment("main"));
        assert!(valid_repo_segment("refs/heads/main"));
        assert!(!valid_repo_segment("owner/repo?x=1"));
        assert!(!valid_repo_segment("owner@evil/repo"));
        assert!(!valid_repo_segment("owner:8080/repo"));
        assert!(!valid_repo_segment("owner/../../etc"));
        assert!(!valid_repo_segment("owner//repo"));
        assert!(!valid_repo_segment("a b"));
        assert!(!valid_repo_segment(""));
    }
}
