// LLM Repository service layer with validation and connectivity testing
// Source: react-test/src-tauri/src/api/repositories.rs
// IMPORTANT: All validation logic copied exactly from react-test

use super::{
    models::LlmRepository,
    types::{
        CreateLlmRepositoryRequest, TestRepositoryConnectionRequest, UpdateLlmRepositoryRequest,
    },
};
use crate::common::AppError;

/// Replace any URL userinfo (`https://user:topsecret@host`) with
/// `https://[REDACTED]@host` before logging. Closes
/// 09-llm-repository F-04 (High) on the test-connection path.
fn redact_url_userinfo(url: &str) -> String {
    match url::Url::parse(url) {
        Ok(mut u) => {
            if !u.username().is_empty() || u.password().is_some() {
                let _ = u.set_username("");
                let _ = u.set_password(None);
                format!("{} (userinfo redacted)", u)
            } else {
                u.to_string()
            }
        }
        Err(_) => "[unparseable URL]".to_string(),
    }
}

/// Validate URL format using reqwest URL parser
pub fn validate_url(url: &str) -> Result<(), AppError> {
    // SSRF-safe validation: reject non-allowlisted schemes (file://, ftp://,
    // git://, gopher://, data:), reject private/loopback/link-local IPs
    // (RFC 1918 + 127/8 + 169.254/16 — AWS IMDS), reject URLs embedding
    // credentials. The previous implementation only checked Url::parse
    // succeeded — that admitted every SSRF flagged by 09-llm-repository
    // F-01 + F-03. PUBLIC_HTTP_OR_HTTPS allows both http and https since
    // self-hosted upstreams may not yet have TLS, but blocks all private
    // address space.
    //
    // Debug builds (cargo test / cargo run) relax to DEV_LOCAL so
    // integration tests can point repositories at a wiremock instance
    // on 127.0.0.1 and so a developer can self-host an upstream on
    // their workstation. Release builds keep the strict policy.
    // Same pattern as auth/providers/oauth2::validate_issuer_url and
    // llm_provider/utils.
    let policy = if cfg!(debug_assertions) {
        crate::utils::url_validator::OutboundUrlPolicy::DEV_LOCAL
    } else {
        crate::utils::url_validator::OutboundUrlPolicy::PUBLIC_HTTP_OR_HTTPS
    };
    crate::utils::url_validator::validate_outbound_url(url, &policy)
        .map(|_| ())
        .map_err(|e| AppError::bad_request("INVALID_URL", e.to_string()))
}

/// Validate auth type is one of the allowed types
pub fn validate_auth_type(auth_type: &str) -> Result<(), AppError> {
    let valid_auth_types = ["none", "api_key", "basic_auth", "bearer_token"];
    if valid_auth_types.contains(&auth_type) {
        Ok(())
    } else {
        Err(AppError::bad_request(
            "VALIDATION_ERROR",
            "Invalid authentication type",
        ))
    }
}

/// Validate authentication configuration for create request
/// Max repository name length. Closes 09-llm-repository F-10 (Medium):
/// without this, an admin could store a multi-MB name that the UI
/// renders without escaping → XSS surface; even with escaping the
/// payload is wasteful.
const MAX_REPO_NAME_LEN: usize = 128;

/// Validate a repository name: non-blank, within the length cap, free of
/// control characters.
///
/// The length cap was the ONLY rule here, which left two holes that both
/// surfaced as 500s on `POST /api/llm-repositories/{id}`:
///
/// * a name carrying U+0000 cannot be stored in a Postgres text column at all
///   (`22021 invalid byte sequence for encoding "UTF8": 0x00`), and
///   `AppError::database_error` flattened that into a generic
///   `SYSTEM_INTERNAL_ERROR`;
/// * a blank name was accepted outright, silently storing an unnamed
///   repository that the admin list then renders as an empty row.
///
/// Rejecting every control character (not just NUL) also closes the
/// Trojan-source display spoof the username/assistant-name gates reject —
/// this string is rendered in the repository picker.
pub(crate) fn validate_repository_name(name: &str) -> Result<(), AppError> {
    if name.trim().is_empty() {
        return Err(AppError::bad_request(
            "VALIDATION_ERROR",
            "Repository name cannot be empty",
        ));
    }
    if name.len() > MAX_REPO_NAME_LEN {
        return Err(AppError::bad_request(
            "VALIDATION_ERROR",
            format!("Repository name exceeds {MAX_REPO_NAME_LEN} chars"),
        ));
    }
    if name.chars().any(char::is_control) {
        return Err(AppError::bad_request(
            "VALIDATION_ERROR",
            "Repository name cannot contain control characters",
        ));
    }
    Ok(())
}

/// Bound + validate the optional auth_test_api_endpoint URL. Closes
/// 09-llm-repository F-08 (Medium): the field was unvalidated
/// free-form text stored in DB, then fetched without scheme/host
/// gating — SSRF surface on test-connection paths. Validates via
/// the shared outbound URL allowlist (no file://, no RFC1918, etc.)
/// when present.
pub(crate) fn validate_test_endpoint(endpoint: &Option<String>) -> Result<(), AppError> {
    if let Some(url) = endpoint {
        if url.trim().is_empty() {
            return Ok(());
        }
        if url.len() > 2048 {
            return Err(AppError::bad_request(
                "VALIDATION_ERROR",
                "auth_test_api_endpoint exceeds 2048 chars",
            ));
        }
        // Match the sibling `validate_url`: DEV_LOCAL only under debug (so the
        // loopback test seams + wiremock fixtures keep working), PUBLIC_HTTP_OR_HTTPS
        // in release — which blocks loopback/RFC1918/link-local. The previous
        // unconditional DEV_LOCAL permitted loopback in a RELEASE build, and the
        // handler comment claiming loopback was blocked was simply false. Link-local
        // (169.254.0.0/16, the IMDS range) is blocked under BOTH policies, so an IMDS
        // endpoint is refused in debug and release alike.
        let policy = if cfg!(debug_assertions) {
            crate::utils::url_validator::OutboundUrlPolicy::DEV_LOCAL
        } else {
            crate::utils::url_validator::OutboundUrlPolicy::PUBLIC_HTTP_OR_HTTPS
        };
        crate::utils::url_validator::validate_outbound_url(url, &policy).map_err(|e| {
            AppError::bad_request(
                "VALIDATION_ERROR",
                format!("auth_test_api_endpoint invalid: {}", e),
            )
        })?;
    }
    Ok(())
}

/// Ensures all required fields are present based on auth_type
pub fn validate_auth_config_for_create(
    request: &CreateLlmRepositoryRequest,
) -> Result<(), AppError> {
    // Bound + sanity-check the repository name (09-llm-repository F-10).
    validate_repository_name(&request.name)?;
    // Validate auth_test_api_endpoint when set (09-llm-repository F-08).
    if let Some(ac) = &request.auth_config {
        validate_test_endpoint(&ac.auth_test_api_endpoint)?;
    }

    // If auth_type is not "none", auth_config must be provided
    if request.auth_type != "none" && request.auth_config.is_none() {
        return Err(AppError::bad_request(
            "VALIDATION_ERROR",
            "Authentication configuration is required for non-none authentication types",
        ));
    }

    // Validate required fields based on auth type
    if let Some(auth_config) = &request.auth_config {
        match request.auth_type.as_str() {
            "api_key" => {
                if auth_config.api_key.is_none()
                    || auth_config.api_key.as_ref().unwrap().trim().is_empty()
                {
                    return Err(AppError::bad_request(
                        "VALIDATION_ERROR",
                        "API key is required for api_key authentication",
                    ));
                }
            }
            "basic_auth" => {
                if auth_config.username.is_none()
                    || auth_config.username.as_ref().unwrap().trim().is_empty()
                    || auth_config.password.is_none()
                    || auth_config.password.as_ref().unwrap().trim().is_empty()
                {
                    return Err(AppError::bad_request(
                        "VALIDATION_ERROR",
                        "Username and password are required for basic_auth authentication",
                    ));
                }
            }
            "bearer_token" => {
                if auth_config.token.is_none()
                    || auth_config.token.as_ref().unwrap().trim().is_empty()
                {
                    return Err(AppError::bad_request(
                        "VALIDATION_ERROR",
                        "Bearer token is required for bearer_token authentication",
                    ));
                }
            }
            _ => {} // "none" requires no additional validation
        }
    }

    Ok(())
}

/// Validate authentication configuration for update request
/// Merges current repository auth with new auth fields and validates the result
pub fn validate_auth_config_for_update(
    current_repository: &LlmRepository,
    request: &UpdateLlmRepositoryRequest,
) -> Result<(), AppError> {
    // Mirror create-time bounds (09-llm-repository F-08/F-10).
    if let Some(name) = &request.name {
        validate_repository_name(name)?;
    }
    if let Some(ac) = &request.auth_config {
        validate_test_endpoint(&ac.auth_test_api_endpoint)?;
    }

    // Determine which auth_type to use (new or current)
    let auth_type = request
        .auth_type
        .as_ref()
        .unwrap_or(&current_repository.auth_type);

    // If auth_config is being updated, validate it
    if let Some(new_auth_config) = &request.auth_config {
        match auth_type.as_str() {
            "api_key" => {
                // Check if new config has api_key
                if new_auth_config.api_key.is_none()
                    || new_auth_config.api_key.as_ref().unwrap().trim().is_empty()
                {
                    // Check if current repository has api_key
                    if current_repository.auth_config.api_key.is_none()
                        || current_repository
                            .auth_config
                            .api_key
                            .as_ref()
                            .unwrap()
                            .trim()
                            .is_empty()
                    {
                        return Err(AppError::bad_request(
                            "VALIDATION_ERROR",
                            "API key is required for api_key authentication",
                        ));
                    }
                }
            }
            "basic_auth" => {
                // Merge username and password from new and current
                let username = new_auth_config
                    .username
                    .as_ref()
                    .or(current_repository.auth_config.username.as_ref());
                let password = new_auth_config
                    .password
                    .as_ref()
                    .or(current_repository.auth_config.password.as_ref());

                if username.is_none()
                    || username.unwrap().trim().is_empty()
                    || password.is_none()
                    || password.unwrap().trim().is_empty()
                {
                    return Err(AppError::bad_request(
                        "VALIDATION_ERROR",
                        "Username and password are required for basic_auth authentication",
                    ));
                }
            }
            "bearer_token" => {
                // Merge token from new and current
                let token = new_auth_config
                    .token
                    .as_ref()
                    .or(current_repository.auth_config.token.as_ref());

                if token.is_none() || token.unwrap().trim().is_empty() {
                    return Err(AppError::bad_request(
                        "VALIDATION_ERROR",
                        "Bearer token is required for bearer_token authentication",
                    ));
                }
            }
            _ => {} // "none" requires no additional validation
        }
    }

    Ok(())
}

// =====================================================================
// Repository kind + model-registry capability probing
// =====================================================================

/// What a repository URL's **host** says the repository is, and therefore
/// which model-listing surface proves it can actually serve models.
///
/// Derived exactly the way the download path derives it
/// (`llm_model/handlers/repo_files.rs`): an exact host match or a
/// subdomain-suffix match, NEVER a substring of the whole URL. A substring
/// test routes `https://evil.example/huggingface.co` at the Hugging Face
/// branch — which is how the stored bearer token used to reach an
/// attacker-chosen host.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RepositoryKind {
    HuggingFace,
    Github,
    /// A host we cannot classify — a self-hosted mirror, a corporate proxy,
    /// or something that is not a model registry at all. Probed against the
    /// Hugging-Face-compatible `/api/models` convention on its own origin;
    /// a host that does not answer with a model listing is reported
    /// `unverified`, never `healthy` and never auto-disabled.
    Unknown,
}

/// Host of `url`, lowercased. `None` when the URL does not parse or carries
/// no host.
fn host_of(url: &str) -> Option<String> {
    url::Url::parse(url)
        .ok()
        .and_then(|u| u.host_str().map(|h| h.to_lowercase()))
}

/// Classify a repository URL by HOST (exact or subdomain suffix).
///
/// Mirrors the `host == suffix || host.ends_with(".{suffix}")` predicate in
/// `llm_model/handlers/repo_files.rs` — the one place in the tree that
/// already answers "what is this repository for".
pub fn repository_kind(url: &str) -> RepositoryKind {
    let Some(host) = host_of(url) else {
        return RepositoryKind::Unknown;
    };
    let host_is = |suffix: &str| host == suffix || host.ends_with(&format!(".{suffix}"));
    if host_is("huggingface.co") {
        RepositoryKind::HuggingFace
    } else if host_is("github.com") {
        RepositoryKind::Github
    } else {
        RepositoryKind::Unknown
    }
}

/// Is this URL a GitHub origin a model download can actually CLONE from?
///
/// `GitService::build_repository_url` turns a GitHub-kind repository row into
/// `{url}/{path}.git`, so only the web origin works as a git remote. The API
/// host (`api.github.com`) answers the REST catalogue but is not a git remote,
/// and it shares the `.github.com` suffix that decides the kind — so without
/// this the capability probe would confirm the API and call the row verified.
fn is_clonable_github_origin(url: &str) -> bool {
    host_of(url).is_some_and(|h| h == "github.com" || h == "www.github.com") && has_no_path(url)
}

/// Does this URL carry no meaningful path — i.e. is it an ORIGIN?
///
/// `GitService::build_repository_url` appends `{path}` to the repository's URL,
/// so for a hosted registry only an origin is a usable base. A row at
/// `https://huggingface.co/custom` builds `https://huggingface.co/custom/org/model`,
/// which is not a repository.
fn has_no_path(url: &str) -> bool {
    url::Url::parse(url)
        .ok()
        .is_some_and(|u| u.path() == "/" || u.path().is_empty())
}

/// Is this URL a usable BASE for the kind's clone/download convention?
///
/// This guard constrains the row's URL SHAPE. It is no longer the only thing
/// standing between a bogus row and `healthy`: `capability_probe_url` now grades
/// the row rather than the host — HuggingFace filters the listing by the row's own
/// org (`?author=<org>`) and `Unknown` probes the row's own PATH — so a row for a
/// nonexistent org yields an empty listing and reads `unverified`. Before that, the
/// probe queried each kind's fixed API host and attributed the HOST's capability to
/// any row sharing it, which is how `https://huggingface.co/custom` and
/// `https://api.github.com` could read `healthy` while unable to serve a model.
///
/// `Unknown` stays unconstrained here: a self-hosted mirror may legitimately live
/// under a path, that path is what its probe now targets, and reporting it
/// unverified would risk auto-disabling a working deployment (INV-4).
fn is_usable_repository_base(kind: RepositoryKind, url: &str) -> bool {
    match kind {
        RepositoryKind::Github => is_clonable_github_origin(url),
        // Hugging Face is deliberately NOT origin-constrained. An org-scoped
        // base is legitimate: a row at `https://huggingface.co/<org>` plus a
        // repository_path of `<model>` builds `huggingface.co/<org>/<model>`,
        // which is a real model URL — and two pre-existing tests rely on it.
        //
        // The gap this used to record — a row naming a NON-EXISTENT org still
        // confirmed on the strength of the hub's whole catalogue — is CLOSED, by
        // the per-row existence probe `…/api/models?limit=1&author=<org>` that
        // `capability_probe_url` now builds. It is closed there rather than here
        // on purpose: an origin-only guard at this layer was tried and rejected,
        // because it reports every legitimate org-scoped row unverified.
        RepositoryKind::HuggingFace => true,
        RepositoryKind::Unknown => true,
    }
}

/// Hugging Face API base for the capability probe.
///
/// `LLM_REPOSITORY_HF_API_BASE` is a **debug-only** test seam (mirrors
/// `LLM_MODEL_HF_API_BASE`, `WEB_SEARCH_BRAVE_ENDPOINT`,
/// `LIT_SEARCH_*_ENDPOINT`) so a loopback fixture can stand in for
/// `huggingface.co`. The `#[cfg(debug_assertions)]` attribute physically
/// removes the lookup from a release build, so it cannot be set in
/// production.
fn hf_api_base() -> String {
    #[cfg(debug_assertions)]
    if let Ok(v) = std::env::var("LLM_REPOSITORY_HF_API_BASE") {
        if !v.trim().is_empty() {
            return v.trim_end_matches('/').to_string();
        }
    }
    "https://huggingface.co".to_string()
}

/// GitHub REST API base for the capability probe. Debug-only seam, same
/// contract as [`hf_api_base`].
fn github_api_base() -> String {
    #[cfg(debug_assertions)]
    if let Ok(v) = std::env::var("LLM_REPOSITORY_GITHUB_API_BASE") {
        if !v.trim().is_empty() {
            return v.trim_end_matches('/').to_string();
        }
    }
    "https://api.github.com".to_string()
}

/// The URL whose response proves this repository can serve models.
///
/// * Hugging Face — `/api/models?limit=1`, the model listing itself, FILTERED by
///   the org the row names in its own path (`&author=<org>`) when it names one.
///   A row with no path segment (the built-in HF Hub) keeps the unfiltered probe
///   and so confirms the host itself.
/// * GitHub — the REST API root, whose catalog carries the
///   `repository_url` template the download path (`fetch_github_files`)
///   uses to read repo trees. A repository row carries no `owner/repo`
///   (that arrives per-download), so the host-level capability is the most
///   that can honestly be asserted here.
/// * Unknown — the Hugging-Face-compatible `/api/models` convention appended to
///   the repository's OWN PATH (not merely its origin), because that path is the
///   base the download path actually uses; a mirror serving only at its root
///   would otherwise read `healthy` for a row pointing at a subpath.
///
/// `None` when the URL does not parse well enough to build an origin.
pub fn capability_probe_url(kind: RepositoryKind, repo_url: &str) -> Option<String> {
    // Derive the org a HF-style row names in its OWN URL path (first non-empty
    // segment). Filtering the listing by `author=<org>` turns the global-catalogue
    // check into a PER-ROW existence check: a nonexistent org returns an EMPTY
    // listing -> is_model_listing() false -> `unverified`, while a real org lists
    // models -> `healthy`. A row with no path segment (the built-in HF Hub) keeps
    // the global probe and correctly confirms the host itself. This ASKS whether
    // the org lists models rather than assuming a path is illegitimate, so
    // `huggingface.co/<org>` stays healthy — avoiding the origin-only guard a
    // prior attempt was reverted for — and it costs no extra request.
    let author = url::Url::parse(repo_url).ok().and_then(|u| {
        u.path_segments()
            .and_then(|mut s| s.find(|seg| !seg.is_empty()).map(str::to_owned))
    });
    match kind {
        RepositoryKind::HuggingFace => {
            let mut u = url::Url::parse(&format!("{}/api/models", hf_api_base())).ok()?;
            u.query_pairs_mut().append_pair("limit", "1");
            if let Some(a) = &author {
                u.query_pairs_mut().append_pair("author", a);
            }
            Some(u.into())
        }
        RepositoryKind::Github => Some(format!("{}/", github_api_base())),
        RepositoryKind::Unknown => {
            // A self-hosted mirror carries no `author` concept — its PATH is the
            // base the download path uses. Probe `<row-url>/api/models`, preserving
            // that path; collapsing to the origin (the old behaviour) graded a URL
            // the download path never touches, so a mirror serving only at its root
            // read `healthy` for a row pointing at a subpath.
            let mut u = url::Url::parse(repo_url)
                .ok()
                .filter(|u| u.origin().ascii_serialization() != "null")?;
            let base = u.path().trim_end_matches('/').to_string();
            u.set_path(&format!("{base}/api/models"));
            u.set_query(None);
            u.query_pairs_mut().append_pair("limit", "1");
            Some(u.into())
        }
    }
}

/// Does this body carry positive evidence that the host can serve models?
///
/// A Hugging-Face-style listing is a NON-EMPTY JSON array whose first
/// element is a model record: a string `id`/`modelId`/`model_id` plus at
/// least one model-registry marker field. Requiring a marker is what makes
/// this a SHAPE assertion rather than a "did the body parse" assertion — a
/// JSON array of arbitrary `{"id": …}` objects is not a model listing.
///
/// An EMPTY array is deliberately not accepted: a registry asked for one
/// model that returns none is not positive evidence, and `unverified` is
/// the honest outcome for "we looked and could not confirm".
pub fn is_model_listing(body: &serde_json::Value) -> bool {
    const MARKERS: &[&str] = &[
        "modelId",
        "model_id",
        "pipeline_tag",
        "siblings",
        "tags",
        "downloads",
        "likes",
        "library_name",
        "lastModified",
        "last_modified",
        "sha",
        "gated",
        "private",
        "author",
    ];
    let Some(items) = body.as_array() else {
        return false;
    };
    let Some(first) = items.first() else {
        return false;
    };
    let Some(obj) = first.as_object() else {
        return false;
    };
    let has_identifier = ["id", "modelId", "model_id"].iter().any(|k| {
        obj.get(*k)
            .and_then(|v| v.as_str())
            .is_some_and(|s| !s.is_empty())
    });
    has_identifier && MARKERS.iter().any(|m| obj.contains_key(*m))
}

/// Does this body look like the GitHub REST API's endpoint catalog?
///
/// The catalog names `repository_url` (the `…/repos/{owner}/{repo}`
/// template `fetch_github_files` builds on), which is the capability a
/// GitHub-hosted model repository row actually needs. An HTML page or an
/// arbitrary JSON object never carries it.
pub fn is_github_api_catalog(body: &serde_json::Value) -> bool {
    body.get("repository_url")
        .and_then(|v| v.as_str())
        .is_some_and(|s| s.contains("/repos/"))
}

/// Apply the kind's capability predicate to a decoded body.
fn confirms_capability(kind: RepositoryKind, body: &serde_json::Value) -> bool {
    match kind {
        RepositoryKind::Github => is_github_api_catalog(body),
        RepositoryKind::HuggingFace | RepositoryKind::Unknown => is_model_listing(body),
    }
}

/// The three-way outcome of a repository probe.
///
/// The middle variant is the whole point (DEC-12 / INV-4): "we reached the
/// host and could not confirm a model-serving capability" is neither a lie
/// (`healthy`) nor grounds for auto-disabling a working self-hosted
/// deployment (`unhealthy`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProbeVerdict {
    /// A model-registry capability was positively confirmed.
    Healthy,
    /// Reachable, but the model-serving capability could not be confirmed
    /// for this URL shape. NEVER auto-disables the repository.
    Unverified { reason: String },
    /// The host could not be reached, or rejected the credentials.
    Unhealthy { reason: String },
}

/// Cap on the capability-response body we will buffer + parse. Mirrors the
/// `MAX_BODY_BYTES` guard on the repository-file listing path.
const MAX_CAPABILITY_BODY_BYTES: usize = 1024 * 1024;

/// Map a reqwest transport failure to the historical message vocabulary.
fn map_transport_error(e: &reqwest::Error) -> String {
    let error_msg = e.to_string();
    if error_msg.contains("timeout") {
        "Connection timed out".to_string()
    } else if error_msg.contains("dns") {
        format!("DNS resolution failed: {}", e)
    } else if error_msg.contains("connection") {
        format!("Connection failed: {}", e)
    } else {
        format!("Network request failed: {}", e)
    }
}

/// Read at most [`MAX_CAPABILITY_BODY_BYTES`] of a response body without
/// trusting `Content-Length` (a chunked response has none).
async fn read_capped_body(mut response: reqwest::Response) -> Result<Vec<u8>, String> {
    let mut buf: Vec<u8> = Vec::new();
    loop {
        match response.chunk().await {
            Ok(Some(chunk)) => {
                if buf.len() + chunk.len() > MAX_CAPABILITY_BODY_BYTES {
                    buf.extend_from_slice(&chunk[..MAX_CAPABILITY_BODY_BYTES - buf.len()]);
                    break;
                }
                buf.extend_from_slice(&chunk);
            }
            Ok(None) => break,
            Err(e) => return Err(map_transport_error(&e)),
        }
    }
    Ok(buf)
}

/// Attach the repository's credentials to an outbound probe request.
///
/// The Bearer-vs-`X-API-Key` choice keys off the repository's **host**
/// (`repository_kind`), not `url.contains("huggingface.co")`. The old
/// substring test handed the Hugging Face bearer token to any URL merely
/// CONTAINING that string — e.g. `https://evil.example/huggingface.co`.
fn apply_auth(
    mut req_builder: reqwest::RequestBuilder,
    request: &TestRepositoryConnectionRequest,
) -> reqwest::RequestBuilder {
    let Some(auth_config) = &request.auth_config else {
        return req_builder;
    };
    match request.auth_type.as_str() {
        "api_key" => {
            if let Some(api_key) = &auth_config.api_key {
                if repository_kind(&request.url) == RepositoryKind::HuggingFace {
                    // Hugging Face takes the key as a Bearer token.
                    req_builder =
                        req_builder.header("Authorization", format!("Bearer {}", api_key));
                } else {
                    // For other APIs, use X-API-Key header (common pattern)
                    req_builder = req_builder.header("X-API-Key", api_key);
                }
            }
        }
        "basic_auth" => {
            if let (Some(username), Some(password)) = (&auth_config.username, &auth_config.password)
            {
                req_builder = req_builder.basic_auth(username, Some(password));
            }
        }
        "bearer_token" => {
            if let Some(token) = &auth_config.token {
                req_builder = req_builder.header("Authorization", format!("Bearer {}", token));
            }
        }
        _ => {} // "none" - no authentication
    }
    req_builder
}

/// Probe a repository's model-serving CAPABILITY.
///
/// Two steps, in order:
///
/// 1. **Credentials** — when `auth_test_api_endpoint` is configured, GET it.
///    Anything other than 200 is `unhealthy`, exactly as before: a stale
///    token is an actionable failure and must keep auto-disabling the row.
/// 2. **Capability** — GET the kind's model-listing surface
///    ([`capability_probe_url`]) and require the body to parse into the
///    shape a model listing has ([`confirms_capability`]).
///
/// Outcome vocabulary:
///
/// | observation | verdict |
/// |---|---|
/// | transport failure (DNS / connect / timeout) | `unhealthy` |
/// | HTTP 401 / 403 | `unhealthy` |
/// | 2xx whose body confirms the capability | `healthy` |
/// | anything else (2xx wrong shape, 404, 429, 5xx) | `unverified` |
///
/// The last row is deliberately NOT `unhealthy`: "we reached it and could
/// not confirm" must never auto-disable a working self-hosted deployment
/// (INV-4). Reachability alone is likewise never `healthy` — a web server
/// answering 200 to every GET (a Vite dev server's SPA fallback, the
/// reported `http://127.0.0.1:<vite>/models` case) lands on `unverified`.
///
/// NOTE: this still validates REST-API reachability/credentials, NOT the
/// git-clone Basic-over-HTTPS path that the actual model download uses (see
/// the credential callbacks in `utils/git/service.rs`). For
/// `auth_type == "basic_auth"` the two wire formats differ, so a green
/// "Test connection" does not by itself guarantee the clone will
/// authenticate. What changed is the outcome vocabulary: `healthy` now
/// requires positive evidence rather than a socket answering 200.
pub async fn test_repository_connectivity(
    request: &TestRepositoryConnectionRequest,
) -> ProbeVerdict {
    // Create a reqwest client with timeout (10s for faster feedback).
    // A non-empty User-Agent is REQUIRED: GitHub's REST API (the seeded GitHub
    // test endpoint https://api.github.com/user) rejects any UA-less request with
    // 403 Forbidden *before* checking the token, so a valid token would otherwise
    // fail the connection test with a misleading 403.
    // SSRF guard: route through a client whose DNS resolver + redirect policy
    // re-validate every connect/hop against PUBLIC_HTTP_OR_HTTPS (blocks
    // loopback / RFC1918 / link-local IMDS, and closes the redirect-to-internal
    // bypass). The 10s timeout + User-Agent are applied per-request below since
    // the validated-client builder owns the resolver/redirect config.
    let client = match crate::utils::url_validator::build_validated_client(
        crate::utils::url_validator::OutboundUrlPolicy::PUBLIC_HTTP_OR_HTTPS,
    ) {
        Ok(c) => c,
        Err(e) => {
            return ProbeVerdict::Unhealthy {
                reason: format!("Failed to create HTTP client: {}", e),
            };
        }
    };

    let get = |url: &str| {
        // Timeout + UA are per-request (the validated client owns the
        // resolver/redirect config). GitHub's REST API rejects UA-less
        // requests with 403 before checking the token.
        let builder = client
            .get(url)
            .timeout(std::time::Duration::from_secs(10))
            .header(
                reqwest::header::USER_AGENT,
                concat!("ziee/", env!("CARGO_PKG_VERSION")),
            );
        apply_auth(builder, request)
    };

    // ── Step 1: credentials ──────────────────────────────────────────
    // Only runs when an explicit auth-test endpoint is configured (the
    // seeded Hugging Face `whoami-v2` / GitHub `/user` rows). Its
    // semantics are UNCHANGED: anything but 200 is a real failure.
    let credential_endpoint = request
        .auth_config
        .as_ref()
        .and_then(|c| c.auth_test_api_endpoint.as_ref())
        .map(|s| s.trim())
        .filter(|s| !s.is_empty());

    if let Some(endpoint) = credential_endpoint {
        tracing::info!(
            "Testing repository credentials against: {}",
            redact_url_userinfo(endpoint)
        );
        match get(endpoint).send().await {
            Ok(response) if response.status() == 200 => {}
            Ok(response) => {
                return ProbeVerdict::Unhealthy {
                    reason: format!("HTTP request failed with status: {}", response.status()),
                };
            }
            Err(e) => {
                return ProbeVerdict::Unhealthy {
                    reason: map_transport_error(&e),
                };
            }
        }
    }

    // ── Step 2: model-registry capability ────────────────────────────
    let kind = repository_kind(&request.url);

    // A GitHub-kind row whose own URL is NOT a clonable origin cannot serve
    // models even though the GitHub REST API answers for it. `api.github.com`
    // is the reported case: it shares the `.github.com` suffix, so the kind
    // probe would confirm the REST catalogue and report `healthy` — while the
    // download path builds `{repo.url}/{path}.git`, i.e.
    // `https://api.github.com/owner/repo.git`, which is not a git remote.
    // Confirming the API here would reproduce exactly the lie INV-4 forbids,
    // so the row is reported `unverified` rather than verified-and-broken.
    if !is_usable_repository_base(kind, &request.url) {
        return ProbeVerdict::Unverified {
            reason: format!(
                "{} is not a usable repository origin for this host — downloads append \
                 the model path to it (e.g. https://huggingface.co/<org>/<model>, \
                 https://github.com/<owner>/<repo>.git), so this URL could not be \
                 confirmed as a model repository",
                redact_url_userinfo(&request.url)
            ),
        };
    }

    let Some(capability_url) = capability_probe_url(kind, &request.url) else {
        return ProbeVerdict::Unverified {
            reason: format!(
                "Could not derive a model-listing URL from {}",
                redact_url_userinfo(&request.url)
            ),
        };
    };

    tracing::info!(
        "Testing model-registry capability at: {}",
        redact_url_userinfo(&capability_url)
    );

    let response = match get(&capability_url).send().await {
        Ok(r) => r,
        Err(e) => {
            return ProbeVerdict::Unhealthy {
                reason: map_transport_error(&e),
            };
        }
    };

    let status = response.status();
    if status == reqwest::StatusCode::FORBIDDEN {
        // 403, and ONLY 403, degrades to `unverified` rather than `unhealthy`.
        //
        // `Unhealthy` is the only verdict that AUTO-DISABLES the row, and 403 is
        // exactly what GitHub returns when the anonymous 60-req/hr/IP budget is
        // exhausted — the shared-egress condition this codebase documents as
        // routine. Disabling a working repository because an unrelated budget
        // ran out is the outcome INV-4 forbids for a repository we merely could
        // not confirm.
        //
        // 401 keeps its historical `unhealthy` treatment below: an explicit
        // authentication challenge against a URL the operator configured is a
        // real, actionable failure, not an unknown.
        return ProbeVerdict::Unverified {
            reason: format!(
                "{} answered HTTP 403 (rate-limited, or access is forbidden), so \
                 this URL could not be confirmed as a model repository",
                redact_url_userinfo(&capability_url),
            ),
        };
    }
    if status == reqwest::StatusCode::UNAUTHORIZED {
        // A credential failure is actionable and keeps its historical
        // treatment (record unhealthy → auto-disable).
        return ProbeVerdict::Unhealthy {
            reason: format!("HTTP request failed with status: {}", status),
        };
    }
    if !status.is_success() {
        return ProbeVerdict::Unverified {
            reason: format!(
                "{} did not serve a model listing (HTTP {})",
                redact_url_userinfo(&capability_url),
                status
            ),
        };
    }

    let body = match read_capped_body(response).await {
        Ok(b) => b,
        Err(reason) => return ProbeVerdict::Unverified { reason },
    };
    let Ok(json) = serde_json::from_slice::<serde_json::Value>(&body) else {
        return ProbeVerdict::Unverified {
            reason: format!(
                "{} answered {} but the response is not JSON, so no model listing could be \
                 confirmed",
                redact_url_userinfo(&capability_url),
                status
            ),
        };
    };
    if confirms_capability(kind, &json) {
        ProbeVerdict::Healthy
    } else {
        ProbeVerdict::Unverified {
            reason: format!(
                "{} answered {} but the response is not a model listing, so this URL could not \
                 be confirmed as a model repository",
                redact_url_userinfo(&capability_url),
                status
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repository_name_accepts_ordinary_names() {
        for n in ["hf", "Hugging Face", "my-mirror.internal", "モデル置き場"] {
            assert!(validate_repository_name(n).is_ok(), "input {n:?}");
        }
        // Exactly at the cap is legal.
        assert!(validate_repository_name(&"a".repeat(MAX_REPO_NAME_LEN)).is_ok());
    }

    #[test]
    fn repository_name_rejects_blank_over_long_and_control_bearing() {
        for n in ["", "   ", "bad\u{0}name", "line\nbreak"] {
            let err = validate_repository_name(n).expect_err("expected rejection");
            assert_eq!(err.status_code(), 400, "input {n:?}");
            assert_eq!(err.error_code(), "VALIDATION_ERROR", "input {n:?}");
        }
        let err = validate_repository_name(&"a".repeat(MAX_REPO_NAME_LEN + 1))
            .expect_err("over-cap name must be rejected");
        assert_eq!(err.status_code(), 400);
        assert_eq!(err.error_code(), "VALIDATION_ERROR");
    }

    // ── TEST-20 (ITEM-13) ────────────────────────────────────────────
    // Host-kind derivation is a HOST-suffix match, not a substring test.
    // The substring test is what handed the stored Hugging Face bearer
    // token to `https://evil.example/huggingface.co`. True positives ship
    // in the SAME test so a rejection that passes because the predicate
    // rejects everything cannot go unnoticed.
    #[test]
    fn repository_kind_matches_host_suffix_not_substring() {
        // True positives — exact host and subdomain.
        for url in [
            "https://huggingface.co",
            "https://huggingface.co/custom",
            "https://sub.huggingface.co/api/models",
            "https://HuggingFace.CO/",
        ] {
            assert_eq!(
                repository_kind(url),
                RepositoryKind::HuggingFace,
                "expected HuggingFace for {url:?}"
            );
        }

        // The bug: the string appears in the URL but NOT as the host.
        for url in [
            "https://evil.example/huggingface.co",
            "https://huggingface.co.evil.example",
            "https://huggingface.co.evil.example/api/models",
            "https://nothuggingface.co",
        ] {
            assert_ne!(
                repository_kind(url),
                RepositoryKind::HuggingFace,
                "must NOT route the Hugging Face bearer token at {url:?}"
            );
        }

        // GitHub gets the same treatment (exact + subdomain + negatives).
        assert_eq!(
            repository_kind("https://github.com"),
            RepositoryKind::Github
        );
        assert_eq!(
            repository_kind("https://api.github.com/user"),
            RepositoryKind::Github
        );
        assert_ne!(
            repository_kind("https://evil.example/github.com"),
            RepositoryKind::Github
        );
        assert_ne!(
            repository_kind("https://github.com.evil.example"),
            RepositoryKind::Github
        );

        // Anything else is Unknown — including an unparseable URL.
        assert_eq!(
            repository_kind("https://models.internal.corp"),
            RepositoryKind::Unknown
        );
        assert_eq!(
            repository_kind("http://127.0.0.1:1520/models"),
            RepositoryKind::Unknown
        );
        assert_eq!(repository_kind("not a url"), RepositoryKind::Unknown);
    }

    /// The auth-header choice must follow the SAME host predicate, or the
    /// substring bug is only half-fixed. Locks the branch condition.
    #[test]
    fn hugging_face_bearer_branch_follows_host_not_substring() {
        assert!(
            repository_kind("https://huggingface.co/custom") == RepositoryKind::HuggingFace,
            "canonical host takes the Bearer branch"
        );
        assert!(
            repository_kind("https://evil.example/huggingface.co") != RepositoryKind::HuggingFace,
            "a path segment must take the X-API-Key branch, never Bearer"
        );
    }

    // ── TEST-21 (ITEM-11) ────────────────────────────────────────────
    // The capability predicate is a SHAPE assertion, not a "did the body
    // parse as JSON" assertion.
    #[test]
    fn model_listing_predicate_accepts_real_payload_and_rejects_lookalikes() {
        // A trimmed but faithful Hugging Face `/api/models` payload.
        let hf: serde_json::Value = serde_json::from_str(
            r#"[
                {
                    "_id": "621ffdc036468d709f17434d",
                    "id": "openai-community/gpt2",
                    "likes": 2743,
                    "private": false,
                    "downloads": 12345678,
                    "tags": ["transformers", "gpt2", "text-generation"],
                    "pipeline_tag": "text-generation",
                    "modelId": "openai-community/gpt2"
                }
            ]"#,
        )
        .unwrap();
        assert!(is_model_listing(&hf), "a real HF model listing is accepted");

        // An HTML document — the Vite dev-server SPA fallback shape. It is
        // not even JSON, so it can never reach the predicate as a value;
        // the string-as-JSON form is the closest representable case.
        let html = serde_json::Value::String(
            "<!doctype html><html><body><div id=\"root\"></div></body></html>".to_string(),
        );
        assert!(!is_model_listing(&html), "an HTML document is rejected");

        // An empty object.
        assert!(
            !is_model_listing(&serde_json::json!({})),
            "an empty object is rejected"
        );
        // The historical `mock_ok` body, which used to be reported healthy.
        assert!(!is_model_listing(&serde_json::json!({"ok": true})));

        // A JSON ARRAY of non-model objects — parses fine, has ids, but
        // carries no model-registry marker.
        assert!(
            !is_model_listing(&serde_json::json!([{"id": "a"}, {"id": "b"}])),
            "an array of bare id objects is not a model listing"
        );
        assert!(
            !is_model_listing(&serde_json::json!([{"title": "not a model"}])),
            "an array of arbitrary objects is not a model listing"
        );
        // An EMPTY array is not positive evidence.
        assert!(!is_model_listing(&serde_json::json!([])));
    }

    #[test]
    fn github_catalog_predicate_accepts_rest_root_and_rejects_lookalikes() {
        let catalog = serde_json::json!({
            "current_user_url": "https://api.github.com/user",
            "repository_url": "https://api.github.com/repos/{owner}/{repo}",
        });
        assert!(is_github_api_catalog(&catalog));
        assert!(!is_github_api_catalog(&serde_json::json!({})));
        assert!(!is_github_api_catalog(
            &serde_json::json!({"repository_url": 7})
        ));
        assert!(!is_github_api_catalog(&serde_json::json!({"ok": true})));
    }

    /// The reported `https://api.github.com` row. It shares the `.github.com`
    /// suffix that selects the GitHub kind, and the GitHub REST catalogue it
    /// probes genuinely answers — so without an origin guard the capability
    /// probe CONFIRMS it and the row reads "Verified as a model repository",
    /// while `{url}/{path}.git` is not a git remote and no download can work.
    ///
    /// The true-positive half is in the same test so a guard that rejected
    /// every GitHub row would not pass.
    #[test]
    fn only_a_clonable_github_origin_can_be_verified() {
        // Clonable — the seeded GitHub row, and its www alias.
        assert!(is_clonable_github_origin("https://github.com"));
        assert!(is_clonable_github_origin("https://github.com/"));
        assert!(is_clonable_github_origin("https://www.github.com"));

        // NOT clonable — API/other subdomains. These still classify as the
        // GitHub KIND (that is what makes the guard necessary).
        for url in [
            "https://api.github.com",
            "https://api.github.com/",
            "https://raw.github.com",
            "https://gist.github.com",
        ] {
            assert_eq!(
                repository_kind(url),
                RepositoryKind::Github,
                "{url} must still classify as the GitHub kind"
            );
            assert!(
                !is_clonable_github_origin(url),
                "{url} is not a git remote and must not be verifiable"
            );
        }
    }

    /// The THREE URLs reported as wrongly-healthy, pinned together.
    ///
    /// Each is reachable and each is a real host, so no reachability check can
    /// separate them from a working repository. What separates them is that the
    /// download path appends the model path to the row's URL, and none of these
    /// is a usable base for that. Two of them (`huggingface.co/custom`,
    /// `api.github.com`) additionally sit on hosts whose kind-level capability
    /// probe genuinely succeeds — so without this guard the probe CONFIRMS them.
    #[test]
    fn the_three_reported_urls_are_not_usable_repository_bases() {
        // `api.github.com` is the case the base guard closes outright.
        assert!(!is_usable_repository_base(
            repository_kind("https://api.github.com"),
            "https://api.github.com"
        ));
        // The Vite dev server is Unknown-kind, so the base guard passes it —
        // it is rejected LATER, by the capability shape check, because its SPA
        // fallback is HTML rather than a model listing.
        assert_eq!(
            repository_kind("http://127.0.0.1:1520/models"),
            RepositoryKind::Unknown
        );
        // KNOWN GAP: `https://huggingface.co/custom` still passes the base
        // guard (an org-scoped HF base is legitimate) and is then confirmed by
        // the hub-wide catalogue. See is_usable_repository_base's note.
        assert!(is_usable_repository_base(
            repository_kind("https://huggingface.co/custom"),
            "https://huggingface.co/custom"
        ));

        // True-positive counterpart: the two seeded rows ARE usable bases, so
        // the guard discriminates rather than refusing everything.
        assert!(is_usable_repository_base(
            RepositoryKind::HuggingFace,
            "https://huggingface.co"
        ));
        assert!(is_usable_repository_base(
            RepositoryKind::HuggingFace,
            "https://huggingface.co/"
        ));
        assert!(is_usable_repository_base(
            RepositoryKind::Github,
            "https://github.com"
        ));
    }

    #[test]
    fn capability_url_targets_the_kinds_listing_surface() {
        assert_eq!(
            capability_probe_url(RepositoryKind::HuggingFace, "https://huggingface.co").as_deref(),
            Some("https://huggingface.co/api/models?limit=1"),
        );
        // A row WITH a path segment is filtered by that segment as the org, so
        // it no longer inherits huggingface.co's whole catalogue. This is the
        // property that stops a row for a nonexistent org reading `healthy`
        // against a listing the download path never uses.
        assert_eq!(
            capability_probe_url(RepositoryKind::HuggingFace, "https://huggingface.co/custom")
                .as_deref(),
            Some("https://huggingface.co/api/models?limit=1&author=custom"),
        );
        assert_eq!(
            capability_probe_url(RepositoryKind::Github, "https://github.com").as_deref(),
            Some("https://api.github.com/"),
        );
        // An Unknown row is probed on its OWN path, not its origin: the path is
        // the base the download path actually uses, so collapsing to the origin
        // graded a URL that path never touches — a mirror serving only at its
        // root read `healthy` for a row pointing at a subpath.
        assert_eq!(
            capability_probe_url(RepositoryKind::Unknown, "http://127.0.0.1:1520/models")
                .as_deref(),
            Some("http://127.0.0.1:1520/models/api/models?limit=1"),
        );
        // A row with no path keeps the bare-origin probe.
        assert_eq!(
            capability_probe_url(RepositoryKind::Unknown, "http://127.0.0.1:1520").as_deref(),
            Some("http://127.0.0.1:1520/api/models?limit=1"),
        );
        assert_eq!(
            capability_probe_url(RepositoryKind::Unknown, "not a url"),
            None
        );
    }
}
