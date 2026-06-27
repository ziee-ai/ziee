//! Shared `resource_link` persistence for tool results.
//!
//! An MCP `tool` result can carry `resource_link` content blocks pointing at a file the
//! tool produced (a chart / PDF / CSV), not inline text. This module is the single
//! consumer that turns those links into durable file-store artifacts. It is used by the
//! chat MCP path (`chat_extension::mcp`, both the approval and auto-exec sites) and is
//! call-ready for the workflow tool dispatcher (wired by `feat/workflow-standalone-runs`).
//!
//! Per link, [`persist_links`] dispatches on the URI shape:
//!   * `is_saved == Some(true)` — already in the file store; referenced, never re-saved.
//!   * `ziee://<host_abs_path>` — a TRUSTED in-process tool produced a transient file on
//!     the host (today only `code_sandbox` workspace artifacts). The bytes are read
//!     straight off disk (no JWT, no HTTP) behind the three guards below, ingested, and
//!     the link's URI is then rewritten to `/api/files/{id}` so the raw host path never
//!     reaches the client / LLM.
//!   * everything else (`is_saved:false` / `None`, an HTTP(S) URL) — fetched over HTTP
//!     (short-lived JWT for built-in servers, configured headers for external ones) and
//!     ingested. Skipped when no `jwt_secret` is available in this context (the
//!     dispatcher passes `None`).
//!
//! ### The three `ziee://` guards (security-critical — do not remove)
//! 1. **Trusted-emitter only** — a `ziee://` host path is honored ONLY when the producing
//!    server is a deterministic built-in ([`is_trusted_resource_emitter`]). An external /
//!    user-configured server's result content is attacker-influenced, so a `ziee://` from
//!    it is ignored (never read).
//! 2. **Confine + canonicalize** — the path is canonicalized (symlinks resolved) and must
//!    live under one of `allowed_roots`; otherwise it would be an arbitrary host
//!    file-read primitive (`/etc/passwd`, server config, other users' files).
//! 3. **Strip-before-client** — after ingest the link's URI is replaced with
//!    `/api/files/{id}`; the raw host path never survives into a client/LLM payload. Any
//!    `ziee://` link that was NOT ingested (failed save, rejected by a guard, untrusted
//!    emitter) has its URI blanked before this returns, so the host path can never leak.
//!
//! ### `ziee://` scheme note
//! This consumer scheme is ALWAYS `ziee://<absolute-host-path>`. It is intentionally
//! distinct from `workflow_mcp`'s `ziee://workflow-runs/<run>/...` **resource** scheme
//! (a logical handle, served as an MCP `resource` block — NOT a `resource_link` — so it
//! never reaches this function). The non-absolute-path check below enforces the boundary:
//! a `workflow-runs/...` remainder is relative and is rejected outright.

use std::path::{Path, PathBuf};

use uuid::Uuid;

use crate::common::AppError;
use crate::core::repository::Repos;
use crate::modules::mcp::chat_extension::content::ResourceLink;
use crate::modules::mcp::client::manager::McpSessionManager;

/// A file newly ingested from a `resource_link` by [`persist_links`].
#[derive(Debug, Clone)]
pub struct PersistedArtifact {
    /// Index into the `links` slice this artifact was produced from.
    pub link_idx: usize,
    pub file_id: Uuid,
    pub version: i32,
    pub version_id: Uuid,
    pub filename: String,
    pub mime_type: Option<String>,
    pub size: i64,
}

/// Outcome of [`persist_links`].
#[derive(Debug, Default)]
pub struct PersistOutcome {
    /// Files newly ingested this call. Each link's `file_id`/`version`/`version_id` are
    /// already stamped back, and `ziee://` URIs already rewritten to `/api/files/{id}`.
    pub saved: Vec<PersistedArtifact>,
    /// `(display_name, uri)` for `is_saved:true` links — referenced, not re-saved. The
    /// caller surfaces these (e.g. in the chat hidden-content download list).
    pub referenced: Vec<(String, String)>,
}

/// Guard #1: is `server_id` a deterministic built-in (in-process, first-party) server?
///
/// Only these may emit a `ziee://` host path we are willing to read off disk. This is a
/// deliberate SUPERSET of the approval-bypass `chat_extension::mcp::is_builtin_server_id`
/// — it additionally includes `code_sandbox` (the producer of transient artifacts),
/// `skill`, and `workflow`. External / user-configured servers are never trusted here.
pub fn is_trusted_resource_emitter(server_id: Uuid) -> bool {
    server_id == crate::modules::files_mcp::files_mcp_server_id()
        || server_id == crate::modules::memory_mcp::memory_mcp_server_id()
        || server_id == crate::modules::elicitation_mcp::elicitation_mcp_server_id()
        || server_id == crate::modules::bio_mcp::bio_mcp_server_id()
        || server_id == crate::modules::web_search::web_search_server_id()
        || server_id == crate::modules::tool_result_mcp::tool_result_mcp_server_id()
        || server_id == crate::modules::lit_search::lit_search_server_id()
        || server_id == crate::modules::citations::citations_server_id()
        || server_id == crate::modules::code_sandbox::code_sandbox_server_id()
        || server_id == crate::modules::skill_mcp::skill_mcp_server_id()
        || server_id == crate::modules::workflow_mcp::workflow_mcp_server_id()
}

/// Guard #2: canonicalize `path` (resolving symlinks) and require it to live under one of
/// `allowed_roots` (each itself canonicalized). Rejects `..` / symlink escape and any path
/// outside the allowed roots. Returns the canonical path on success.
pub fn confine_under_roots(path: &Path, allowed_roots: &[PathBuf]) -> Result<PathBuf, AppError> {
    let canon_path = std::fs::canonicalize(path)
        .map_err(|_| AppError::not_found("resource file"))?;
    for root in allowed_roots {
        if let Ok(canon_root) = std::fs::canonicalize(root)
            && canon_path.starts_with(&canon_root)
        {
            return Ok(canon_path);
        }
    }
    Err(AppError::forbidden(
        "RESOURCE_LINK_PATH_ESCAPE",
        "resolved path escapes the allowed workspace root",
    ))
}

/// Parse a raw MCP `resource_link` content block into a typed [`ResourceLink`].
///
/// Shared by the chat path's `helpers::execute_tool` and the workflow tool dispatcher so
/// both map the camelCase `mimeType` → `mime_type` field identically. Returns `None` when
/// the block has no `uri`.
pub fn parse_resource_link_block(content: &serde_json::Value) -> Option<ResourceLink> {
    let uri = content.get("uri").and_then(|v| v.as_str())?;
    Some(ResourceLink {
        uri: uri.to_string(),
        name: content.get("name").and_then(|v| v.as_str()).map(String::from),
        mime_type: content
            .get("mimeType")
            .and_then(|v| v.as_str())
            .map(String::from),
        size: content.get("size").and_then(|v| v.as_i64()),
        is_saved: content.get("is_saved").and_then(|v| v.as_bool()),
        // Set for already-saved attachments (the tool emits it); workspace artifacts get
        // it stamped later, after the save pipeline runs.
        file_id: content
            .get("file_id")
            .and_then(|v| v.as_str())
            .and_then(|s| Uuid::parse_str(s).ok()),
        version_id: content
            .get("version_id")
            .and_then(|v| v.as_str())
            .and_then(|s| Uuid::parse_str(s).ok()),
        version: content
            .get("version")
            .and_then(|v| v.as_i64())
            .map(|n| n as i32),
    })
}

/// Is `uri` a `ziee://<absolute-host-path>` (the sensitive host-path dialect this module
/// consumes)? This is the predicate used by every guard-#3 blank/scrub decision, and it is
/// deliberately the SAME test the ingest branch applies (`strip_prefix` + `is_absolute`).
///
/// It is `false` for `ziee://workflow-runs/...` — `workflow_mcp`'s logical resource-handle
/// dialect, whose remainder is relative — so the blanking sweeps and the structured_content
/// scrub leave those handles intact (they are not host paths and carry no disclosure).
pub(crate) fn is_ziee_host_path(uri: &str) -> bool {
    uri.strip_prefix("ziee://")
        .is_some_and(|p| Path::new(p).is_absolute())
}

/// Recursively blank any `ziee://<absolute-host-path>` string anywhere in a JSON value
/// (leaving `ziee://workflow-runs/...` handles untouched — see [`is_ziee_host_path`]).
///
/// Guard #3 defense-in-depth for the `structured_content` channel: a tool's
/// `structuredContent` (e.g. `code_sandbox::get_resource_link`'s output) can embed the raw
/// `ziee://<host_path>`, and that field is persisted as JSONB + shipped to the browser +
/// recalled to the LLM via `tool_result_mcp`. It is never used to ingest, so the host path
/// is scrubbed unconditionally at capture (see `chat_extension::helpers::execute_tool`).
pub fn scrub_ziee_in_value(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::String(s) if is_ziee_host_path(s) => {
            s.clear();
        }
        serde_json::Value::Array(items) => items.iter_mut().for_each(scrub_ziee_in_value),
        serde_json::Value::Object(map) => map.values_mut().for_each(scrub_ziee_in_value),
        _ => {}
    }
}

/// Process `bytes` and persist them as a new `File` owned by `user_id` via the shared
/// `file::ingest::ingest_bytes` save tail (processing pipeline → `save_original` +
/// text/image derivatives → `Repos.file.create` → `publish_file_changed`).
///
/// Returns `(file_id, version, version_id, mime_type, size)`.
async fn save_bytes_as_artifact(
    user_id: Uuid,
    bytes: &[u8],
    display_name: &str,
    content_type_mime: Option<String>,
    message_id: Option<Uuid>,
    created_by: &str,
) -> Result<(Uuid, i32, Uuid, Option<String>, i64), AppError> {
    // E1: the save tail (canonical-extension keying, processing pipeline, blob
    // store, `files`/`file_versions` rows, real checksum, and the cross-device
    // `publish_file_changed` sync) is the SHARED `file::ingest::ingest_bytes` —
    // ONE persistence path across the chat resource_link save, the workflow
    // tool-step save, and run-artifact collection. `workflow_run_id` is `None`
    // here: `persist_links` links the producing run itself, after the save loop
    // populates `outcome.saved` (so the A5 cascade owns only run-created files).
    let file = crate::modules::file::ingest::ingest_bytes(
        user_id,
        bytes,
        display_name,
        content_type_mime,
        created_by,
        message_id,
        None,
    )
    .await?;

    Ok((
        file.id,
        file.version,
        file.current_version_id,
        file.mime_type.clone(),
        file.file_size,
    ))
}

/// Ingest every persistable `resource_link` in `links` into the file store.
///
/// See the module docs for the per-link dispatch + the three `ziee://` guards. The save
/// tail (process → store → DB row → cross-device sync) is identical across URI shapes.
/// `file_id`/`version`/`version_id` are stamped back onto each saved link, and `ziee://`
/// URIs are rewritten to `/api/files/{id}` (guard #3) before this returns.
///
/// When `workflow_run_id` is `Some`, each newly-ingested (run-created) file is linked to
/// that run via `Repos.file.set_workflow_run_id` after the save loop, so the A5 cascade
/// deletes only files the run CREATED (never `is_saved:true` files it referenced). The chat
/// path passes `None`.
#[allow(clippy::too_many_arguments)]
pub async fn persist_links(
    links: &mut [ResourceLink],
    user_id: Uuid,
    conversation_id: Option<Uuid>,
    message_id: Option<Uuid>,
    created_by: &str,
    workflow_run_id: Option<Uuid>,
    server_id: Uuid,
    server_is_built_in: bool,
    server_headers: &serde_json::Value,
    allowed_roots: &[PathBuf],
    jwt_secret: Option<&str>,
) -> Result<PersistOutcome, AppError> {
    // C4: on this branch `files.workflow_run_id` + `Repos.file.set_workflow_run_id`
    // exist, so each newly-ingested file is linked to the producing run below (after
    // the save loop populates `outcome.saved`). A5 cascade-delete removes ONLY the
    // files a run CREATED — never the `is_saved:true` files it merely referenced.
    // The chat path passes `workflow_run_id = None`, so this is a no-op there.

    let mut outcome = PersistOutcome::default();

    for (link_idx, link) in links.iter().enumerate() {
        // is_saved=true: file already exists in originals storage — reference, don't
        // re-save. The URI is a download-with-token URL the caller surfaces as-is.
        if link.is_saved == Some(true) {
            // A genuine is_saved link carries an HTTP(S)/`/api/files` download URL. Never
            // surface a `ziee://` host path through `referenced` (→ chat hidden_content);
            // the blanking sweep below strips such a link's URI regardless.
            if !is_ziee_host_path(&link.uri) {
                let name = link.name.as_deref().unwrap_or("file").to_string();
                outcome.referenced.push((name, link.uri.clone()));
            }
            continue;
        }

        let display_name = link.name.as_deref().unwrap_or("file").to_string();

        // ziee://<host_abs_path>: a trusted in-process producer placed a transient file on
        // the host. Read the bytes directly off disk behind guards #1 and #2.
        if let Some(host_path) = link.uri.strip_prefix("ziee://") {
            // Our scheme is ALWAYS an absolute host path. Reject anything else outright —
            // in particular workflow_mcp's `ziee://workflow-runs/...` resource scheme, whose
            // remainder is relative and must never be treated as a host path. (The blanking
            // sweep at the end strips the leftover ziee:// uri so it can't reach the client.)
            if !Path::new(host_path).is_absolute() {
                tracing::warn!("ignoring non-absolute ziee:// resource_link uri: {}", link.uri);
                continue;
            }
            // Guard #1: trusted emitter only.
            if !is_trusted_resource_emitter(server_id) {
                tracing::warn!(
                    "ignoring ziee:// resource_link from non-trusted server {}: {}",
                    server_id,
                    link.uri
                );
                continue;
            }
            // Guard #2: canonicalize + confine under the allowed root(s).
            let path = match confine_under_roots(Path::new(host_path), allowed_roots) {
                Ok(p) => p,
                Err(e) => {
                    tracing::warn!("ziee:// path rejected ({e}): {}", link.uri);
                    continue;
                }
            };
            let bytes = match tokio::fs::read(&path).await {
                Ok(b) => b,
                Err(e) => {
                    tracing::error!("failed to read ziee:// artifact {}: {e}", path.display());
                    continue;
                }
            };
            match save_bytes_as_artifact(
                user_id,
                &bytes,
                &display_name,
                link.mime_type.clone(),
                message_id,
                created_by,
            )
            .await
            {
                Ok((file_id, version, version_id, mime_type, size)) => {
                    tracing::info!(
                        "Artifact saved from ziee:// resource_link: file_id={file_id}, filename={display_name}"
                    );
                    outcome.saved.push(PersistedArtifact {
                        link_idx,
                        file_id,
                        version,
                        version_id,
                        filename: display_name.clone(),
                        mime_type,
                        size,
                    });
                }
                Err(e) => {
                    tracing::error!("failed to ingest ziee:// artifact '{display_name}': {e}")
                }
            }
            continue;
        }

        // Otherwise: an HTTP(S) URL — an external server's link, or an is_saved:false
        // loopback URL. Requires a JWT secret to mint built-in auth; the dispatcher
        // (which has no config handle) passes None and these are skipped.
        let Some(secret) = jwt_secret else {
            tracing::warn!(
                "skipping HTTP resource_link fetch — no jwt secret in this context: {}",
                link.uri
            );
            continue;
        };

        // Build auth headers appropriate for the server type.
        let mut fetch_headers = reqwest::header::HeaderMap::new();
        if server_is_built_in {
            match McpSessionManager::generate_short_lived_jwt(user_id, secret, 10) {
                Ok(token) => {
                    if let Ok(hval) =
                        reqwest::header::HeaderValue::from_str(&format!("Bearer {}", token))
                    {
                        fetch_headers.insert(reqwest::header::AUTHORIZATION, hval);
                    }
                    if let Some(conv) = conversation_id
                        && let Ok(hval) = reqwest::header::HeaderValue::from_str(&conv.to_string())
                    {
                        fetch_headers.insert(
                            reqwest::header::HeaderName::from_static("x-conversation-id"),
                            hval,
                        );
                    }
                }
                Err(e) => tracing::warn!("Failed to generate JWT for resource_link fetch: {e}"),
            }
        } else if let Some(headers_map) = server_headers.as_object() {
            for (key, value) in headers_map.iter() {
                if let Some(val_str) = value.as_str()
                    && let (Ok(hname), Ok(hval)) = (
                        reqwest::header::HeaderName::from_bytes(key.as_bytes()),
                        reqwest::header::HeaderValue::from_str(val_str),
                    )
                {
                    fetch_headers.insert(hname, hval);
                }
            }
        }

        // A built-in server's link is a trusted loopback URL (127.0.0.1
        // /api/files/...). An external server's link is model-/third-party
        // controlled, so it must be SSRF-confined to public hosts
        // (block loopback / RFC1918 / IMDS) with redirects re-validated.
        let client = if server_is_built_in {
            match reqwest::Client::builder().build() {
                Ok(c) => c,
                Err(e) => {
                    tracing::error!("Failed to build HTTP client for resource_link fetch: {e}");
                    continue;
                }
            }
        } else {
            if let Err(e) = crate::utils::url_validator::validate_outbound_url(
                &link.uri,
                &crate::utils::url_validator::OutboundUrlPolicy::PUBLIC_HTTP_OR_HTTPS,
            ) {
                tracing::error!(
                    "resource_link external fetch rejected by SSRF policy for '{}': {e}",
                    link.uri
                );
                continue;
            }
            match crate::utils::url_validator::build_validated_client(
                crate::utils::url_validator::OutboundUrlPolicy::PUBLIC_HTTP_OR_HTTPS,
            ) {
                Ok(c) => c,
                Err(e) => {
                    tracing::error!("Failed to build HTTP client for resource_link fetch: {e}");
                    continue;
                }
            }
        };
        // Per-request timeout so a slow/stalled peer can't hang persistence.
        let response = match client
            .get(&link.uri)
            .headers(fetch_headers)
            .timeout(std::time::Duration::from_secs(30))
            .send()
            .await
        {
            Ok(r) if r.status().is_success() => r,
            Ok(r) => {
                tracing::error!(
                    "resource_link fetch returned HTTP {} for '{}': artifact NOT saved",
                    r.status(),
                    link.uri
                );
                continue;
            }
            Err(e) => {
                tracing::error!(
                    "Failed to fetch resource_link '{}': {e} — artifact NOT saved",
                    link.uri
                );
                continue;
            }
        };
        let content_type_mime = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.split(';').next())
            .map(|s| s.trim().to_string());
        let bytes = match response.bytes().await {
            Ok(b) => b.to_vec(),
            Err(e) => {
                tracing::warn!("Failed to read resource_link response body: {e}");
                continue;
            }
        };
        match save_bytes_as_artifact(
            user_id,
            &bytes,
            &display_name,
            content_type_mime,
            message_id,
            created_by,
        )
        .await
        {
            Ok((file_id, version, version_id, mime_type, size)) => {
                tracing::info!(
                    "Artifact saved from resource_link: file_id={file_id}, filename={display_name}"
                );
                outcome.saved.push(PersistedArtifact {
                    link_idx,
                    file_id,
                    version,
                    version_id,
                    filename: display_name.clone(),
                    mime_type,
                    size,
                });
            }
            Err(e) => tracing::error!("failed to ingest resource_link '{display_name}': {e}"),
        }
    }

    // Guard #3 + stamping: write file_id/version/version_id back onto each saved link so
    // the UI inline preview fetches via the authenticated /api/files/{id}/... path; for
    // ziee:// links replace the raw host path entirely (it must never reach the client).
    for art in &outcome.saved {
        if let Some(l) = links.get_mut(art.link_idx) {
            l.file_id = Some(art.file_id);
            l.version = Some(art.version);
            l.version_id = Some(art.version_id);
            // A saved link's original URI was a host-path `ziee://` (only those ingest);
            // rewrite it to the authenticated /api path. HTTP-fetched links keep their URI.
            if is_ziee_host_path(&l.uri) {
                l.uri = format!("/api/files/{}", art.file_id);
            }
        }
    }

    // C4: link each run-created file to the producing workflow run so the A5
    // delete-cascade can find them. Only `outcome.saved` (files this call
    // CREATED) — never the referenced `is_saved:true` files.
    if let Some(run) = workflow_run_id {
        for art in &outcome.saved {
            if let Err(e) = Repos.file.set_workflow_run_id(art.file_id, run).await {
                tracing::warn!(
                    "failed to link file {} to workflow run {run}: {e}",
                    art.file_id
                );
            }
        }
    }

    // Guard #3 (defense in depth): a raw `ziee://<host_path>` must NEVER reach the
    // client/LLM. Saved links were rewritten to `/api/files/{id}` above; any host-path link
    // still carrying the scheme here was NOT ingested (failed save, rejected by a guard, or
    // from an untrusted emitter) — blank its URI so the host path can't leak into the
    // persisted `resource_links` the browser reads. (workflow-runs handles are not host
    // paths, so [`is_ziee_host_path`] leaves them intact.)
    for l in links.iter_mut() {
        if is_ziee_host_path(&l.uri) {
            l.uri = String::new();
        }
    }

    Ok(outcome)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trusted_emitter_gate_accepts_builtins_rejects_others() {
        // code_sandbox is the producer — it MUST be trusted (it is NOT in the
        // approval-bypass is_builtin_server_id list, so this is the load-bearing case).
        assert!(is_trusted_resource_emitter(
            crate::modules::code_sandbox::code_sandbox_server_id()
        ));
        assert!(is_trusted_resource_emitter(
            crate::modules::files_mcp::files_mcp_server_id()
        ));
        assert!(is_trusted_resource_emitter(
            crate::modules::lit_search::lit_search_server_id()
        ));
        // A non-built-in (e.g. an external/user server) id is never trusted.
        assert!(!is_trusted_resource_emitter(Uuid::nil()));
        assert!(!is_trusted_resource_emitter(Uuid::from_u128(0xdead_beef)));
    }

    #[test]
    fn parse_resource_link_block_maps_fields() {
        let v = serde_json::json!({
            "type": "resource_link",
            "uri": "ziee:///tmp/ws/out.csv",
            "name": "out.csv",
            "mimeType": "text/csv",
            "size": 12,
            "is_saved": false
        });
        let link = parse_resource_link_block(&v).expect("parsed");
        assert_eq!(link.uri, "ziee:///tmp/ws/out.csv");
        assert_eq!(link.name.as_deref(), Some("out.csv"));
        assert_eq!(link.mime_type.as_deref(), Some("text/csv"));
        assert_eq!(link.size, Some(12));
        assert_eq!(link.is_saved, Some(false));
        // A block with no uri is not a resource_link we can act on.
        assert!(parse_resource_link_block(&serde_json::json!({ "name": "x" })).is_none());
    }

    #[test]
    fn is_ziee_host_path_distinguishes_dialects() {
        // Absolute host-path dialect (the sensitive one) → true.
        assert!(is_ziee_host_path("ziee:///var/lib/ziee/sandboxes/c/out.csv"));
        assert!(is_ziee_host_path("ziee:///etc/passwd"));
        // workflow_mcp's logical resource handle (relative remainder) → false (preserve it).
        assert!(!is_ziee_host_path("ziee://workflow-runs/abc/outputs/x.json"));
        // Non-ziee + malformed → false.
        assert!(!is_ziee_host_path("/api/files/1"));
        assert!(!is_ziee_host_path("https://ok/y"));
        assert!(!is_ziee_host_path("ziee://")); // empty remainder
    }

    #[test]
    fn scrub_ziee_in_value_blanks_host_paths_keeps_workflow_handles() {
        let mut v = serde_json::json!({
            "type": "resource_link",
            "uri": "ziee:///var/lib/ziee/sandboxes/c/out.csv",
            "name": "out.csv",
            "nested": { "u": "ziee:///etc/passwd", "ok": "/api/files/1" },
            "list": ["ziee:///x", "https://ok/y", 7],
            // workflow_mcp's logical handle must survive the scrub (not a host path).
            "workflow_handle": "ziee://workflow-runs/r1/outputs/out",
        });
        scrub_ziee_in_value(&mut v);
        assert_eq!(v["uri"], ""); // host path blanked
        assert_eq!(v["nested"]["u"], ""); // host path blanked
        assert_eq!(v["nested"]["ok"], "/api/files/1"); // non-ziee untouched
        assert_eq!(v["list"][0], ""); // host path blanked
        assert_eq!(v["list"][1], "https://ok/y");
        assert_eq!(v["list"][2], 7);
        assert_eq!(v["workflow_handle"], "ziee://workflow-runs/r1/outputs/out"); // preserved
        // No absolute host-path `ziee://` survives; the workflow handle is allowed.
        assert!(!serde_json::to_string(&v)
            .unwrap()
            .contains("ziee:///"));
    }

    #[test]
    fn confine_accepts_in_root_rejects_outside_and_missing() {
        let base = std::env::temp_dir().join(format!("ziee_rl_confine_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&base);
        let inside = base.join("artifact.txt");
        std::fs::write(&inside, b"hi").unwrap();
        let roots = vec![base.clone()];

        // In-root path is accepted and returns a canonical path under the root.
        let ok = confine_under_roots(&inside, &roots).expect("in-root accepted");
        assert!(ok.starts_with(std::fs::canonicalize(&base).unwrap()));

        // A path outside every allowed root is rejected.
        let outside_dir =
            std::env::temp_dir().join(format!("ziee_rl_outside_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&outside_dir);
        let outside = outside_dir.join("secret.txt");
        std::fs::write(&outside, b"secret").unwrap();
        assert!(confine_under_roots(&outside, &roots).is_err());

        // A non-existent path is rejected (canonicalize fails).
        assert!(confine_under_roots(&base.join("missing.txt"), &roots).is_err());

        let _ = std::fs::remove_dir_all(&base);
        let _ = std::fs::remove_dir_all(&outside_dir);
    }

    #[cfg(unix)]
    #[test]
    fn confine_rejects_symlink_escape() {
        use std::os::unix::fs::symlink;
        let base = std::env::temp_dir().join(format!("ziee_rl_symlink_{}", std::process::id()));
        let target_dir =
            std::env::temp_dir().join(format!("ziee_rl_symtarget_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&base);
        let _ = std::fs::create_dir_all(&target_dir);
        let secret = target_dir.join("passwd");
        std::fs::write(&secret, b"root:x:0:0").unwrap();
        let link = base.join("escape");
        let _ = std::fs::remove_file(&link);
        symlink(&secret, &link).unwrap();

        // canonicalize() resolves the symlink to target_dir/passwd, which is outside the
        // allowed root → rejected.
        assert!(confine_under_roots(&link, &[base.clone()]).is_err());

        let _ = std::fs::remove_dir_all(&base);
        let _ = std::fs::remove_dir_all(&target_dir);
    }
}
