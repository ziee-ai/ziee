// MCP chat extension implementation

use aide::axum::ApiRouter;
use async_trait::async_trait;
use axum::response::sse::Event;
use serde_json::Value;
use sqlx::PgPool;
use std::collections::HashMap;
use std::convert::Infallible;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

use ai_providers::{ChatRequest, ContentBlock};

use crate::common::AppError;
use crate::modules::chat::core::extension::{
    BeforeLlmAction, ChatExtension, ExtensionAction, SendMessageRequest, StreamContext,
};
use crate::modules::chat::core::models::{Message, MessageContentData};
use crate::modules::chat::core::types::streaming::ContentBlockDelta;
use crate::modules::mcp::client::manager::McpSessionManager;
use crate::modules::mcp::client::session::McpSession;
use crate::modules::mcp::UsageMode;
use crate::modules::mcp::sampling::{ChatSamplingHandler, acquire_session};
use crate::modules::mcp::elicitation::models::ElicitationStartedNotification;
use crate::core::repository::Repos;

use super::content::McpContentData;
use super::helpers;

/// Origin (`scheme://host[:port]`) for file download URLs handed to the LLM
/// for tool-to-tool transfer of saved artifacts.
///
/// Resolves to `code_sandbox.public_base_url` when configured, otherwise the
/// pinned `127.0.0.1` loopback. Deliberately does NOT consult
/// `server.host`: that value can be `0.0.0.0` / a wildcard / a bind address
/// that is not a routable destination, and handing such a URL to a (possibly
/// remote) MCP server is exactly the bug this fixes. The loopback is always
/// `127.0.0.1` — matching `code_sandbox::loopback_host` and the origin
/// `get_resource_link` returns — so the two paths can never drift.
///
/// Pure (no `self`, no I/O) so it is directly unit-testable.
fn file_download_origin(
    code_sandbox: Option<&crate::core::config::CodeSandboxConfig>,
    server_port: u16,
) -> String {
    let loopback_origin = format!("http://127.0.0.1:{server_port}");
    code_sandbox
        .map(|cs| cs.public_file_origin(&loopback_origin))
        .unwrap_or(loopback_origin)
}

/// Build the tool-to-tool download URL for a saved MCP artifact. `origin` must
/// already be resolved via [`file_download_origin`]. Pure so the URL shape
/// (and token preservation) is unit-testable without a live extension.
fn build_artifact_download_url(
    origin: &str,
    api_prefix: &str,
    artifact_id: Uuid,
    token: &str,
) -> String {
    // Trim a trailing slash off api_prefix so a config value like "/api/"
    // can't yield a double slash ("…/api//files/…"). Mirrors the guard in
    // llm_local_runtime::proxy::derive_proxy_url.
    let api_prefix = api_prefix.trim_end_matches('/');
    format!("{origin}{api_prefix}/files/{artifact_id}/download-with-token?token={token}")
}

/// The iteration-1 system-message addition for tool usage.
///
/// Always includes the "prefer tools over training knowledge" nudge.
/// Additionally includes the file-URL rule WHEN `get_resource_link` is among
/// the available tools: this promotes the rule from the tool description
/// (weak, reactive) to a system instruction (strong, proactive, issued before
/// the first tool call), because the model otherwise tends to fabricate a
/// plausible file/download URL (e.g. a platform or DRS endpoint) instead of
/// calling the tool. Gated on the tool actually being present so we never
/// instruct the model to call a tool it doesn't have — tool names are
/// `{server_id}__{tool}` (see `helpers::convert_mcp_tool_to_ai_tool`).
///
/// Pure (no `self`, no I/O) so it is directly unit-testable.
fn tool_system_guidance(tools: &[ai_providers::Tool]) -> String {
    let mut guidance = String::from(
        "\n\nYou have access to tools that can retrieve up-to-date or domain-specific \
         information. When answering questions, prefer using these tools over relying solely \
         on your training knowledge, especially when the tools are clearly relevant to the request.",
    );
    if tools
        .iter()
        .any(|t| t.function.name.ends_with("__get_resource_link"))
    {
        guidance.push_str(
            "\n\nTo give any tool a URL or path for a file the user attached or that you \
             produced, you MUST first call get_resource_link to obtain its download URL, then \
             pass that URL verbatim. Never invent, guess, or construct a file/download URL \
             (e.g. a platform or DRS endpoint) — these files are reachable ONLY via the URL \
             get_resource_link returns.",
        );
    }
    guidance
}

/// Accumulated tool use data during streaming
#[derive(Debug, Clone, Default)]
struct AccumulatedToolUse {
    id: Option<String>,
    name: Option<String>,
    input_json: String, // Accumulated JSON string
}

/// MCP chat extension
/// Deterministic ids of the privileged built-in MCP servers to auto-attach this
/// request. `files`/`memory`/`web_search`/`lit_search` attach behind flags set by
/// the file (`attach_files_mcp`), memory (`attach_memory_mcp`), web_search
/// (`attach_web_search_mcp`), and lit_search (`attach_lit_search_mcp`) chat
/// extensions; `elicitation` (`ask_user`) and `tool_result` (`get_tool_result`)
/// attach whenever the model is tool-capable (`model_tools_capable`). All are
/// fetched by id OUTSIDE the group-gated accessibility path — no per-user grant —
/// and only for tool-capable models.
fn auto_attach_builtin_ids(
    metadata: &std::collections::HashMap<String, serde_json::Value>,
) -> Vec<Uuid> {
    let flag = |k: &str| {
        metadata
            .get(k)
            .and_then(|v| v.as_str())
            .map(|s| s == "true")
            .unwrap_or(false)
    };
    let mut ids = Vec::new();
    if flag("attach_files_mcp") {
        ids.push(crate::modules::files_mcp::files_mcp_server_id());
    }
    if flag("attach_memory_mcp") {
        ids.push(crate::modules::memory_mcp::memory_mcp_server_id());
    }
    // `bio` attaches behind a flag set by the bio_mcp chat extension
    // (`attach_bio_mcp`), gated on the model being tool-capable AND the
    // admin having enabled the bio row. Like the others it's fetched by
    // id OUTSIDE the group-gated path; the `s.enabled` guard at the
    // fetch site (and the bio extension's own check) keeps a disabled
    // bio off.
    if flag("attach_bio_mcp") {
        ids.push(crate::modules::bio_mcp::bio_mcp_server_id());
    }
    // `web_search` attaches behind the flag set by the web_search chat
    // extension (`attach_web_search_mcp`), gated on tool-capable + enabled +
    // ≥1 configured provider in the chain. Same id-fetch + `s.enabled` guard.
    if flag(crate::modules::web_search::chat_extension::ATTACH_FLAG) {
        ids.push(crate::modules::web_search::web_search_server_id());
    }
    // `lit_search` attaches behind the flag set by the lit_search chat extension
    // (`attach_lit_search_mcp`), gated on tool-capable + enabled. Same id-fetch +
    // `s.enabled` guard.
    if flag(crate::modules::lit_search::chat_extension::ATTACH_FLAG) {
        ids.push(crate::modules::lit_search::lit_search_server_id());
    }
    // `ask_user` is always-on — the assistant may need to ask the user for input
    // in any conversation — but ONLY for tool-capable models: a model that can't
    // call tools can't call `ask_user`, and attaching it would run the full
    // before_llm_call body (loopback session + tools/list) on EVERY chat, incl.
    // non-tool-capable models and MCP-off chats. The flag-gated built-ins above
    // are already only flagged on the tool-capable path (file.rs gates
    // `attach_files_mcp` on `tool_capable`); mirror that contract here.
    // `model_tools_capable` is memoized into metadata by
    // chat/core/services/streaming.rs before the extension pipeline runs (and may
    // round-trip as a JSON bool or "true"/"false" string). Auto-approved (the
    // user answering the form IS the approval); execution is intercepted in
    // `helpers::execute_tool`, not dispatched over the loopback.
    let tool_capable = metadata
        .get("model_tools_capable")
        .and_then(|v| v.as_bool().or_else(|| v.as_str().map(|s| s == "true")))
        .unwrap_or(false);
    if tool_capable {
        ids.push(crate::modules::elicitation_mcp::elicitation_mcp_server_id());
        // `get_tool_result` is always-on for tool-capable models — the model may
        // need to recall a cleared/truncated tool result (the trimming placeholder
        // points it here) or read an earlier result's full structuredContent in
        // ANY tool-using conversation. Read-only, scoped to the caller's own
        // conversation; approval-bypassed (see is_builtin_server_id).
        ids.push(crate::modules::tool_result_mcp::tool_result_mcp_server_id());
    }
    ids
}

/// Side-effect tools don't produce a result the model needs to reason about, so
/// when ONLY these were called in an iteration the tool-use loop finalizes
/// without a no-op continuation round-trip (Track B inline self-save).
///
/// Scoped to the memory built-in server id — a third-party MCP server that
/// happens to expose a tool NAMED `remember`/`forget` is NOT side-effect (its
/// result may well be something the model needs to reason about, so the loop
/// must continue as usual). Only the privileged built-in memory tools qualify.
fn is_side_effect_tool(server_id: Uuid, tool_name: &str) -> bool {
    server_id == crate::modules::memory_mcp::memory_mcp_server_id()
        && matches!(tool_name, "remember" | "forget")
}

/// Privileged built-in servers (files, memory, elicitation, bio, web_search,
/// lit_search, tool_result). Their tools bypass the MCP approval flow — they're
/// read-only / save-only / user-prompting and auto-attached, so a
/// `read_file`/`remember`/`web_search`/`literature_search`/`get_tool_result`/
/// `ask_user` call must execute immediately rather than stall behind a
/// manual-approval prompt the user never opted into (for `ask_user`, the user
/// answering the form IS the approval).
fn is_builtin_server_id(id: Uuid) -> bool {
    id == crate::modules::files_mcp::files_mcp_server_id()
        || id == crate::modules::memory_mcp::memory_mcp_server_id()
        || id == crate::modules::elicitation_mcp::elicitation_mcp_server_id()
        // bio is approval-bypassed (read-only biomedical searches, auto-attached)
        // but — unlike the three above — it is NOT in the zero-config edit
        // deny-list (`repository.rs::update_system_mcp_server`), so admins can
        // still edit its Headers (API keys). The two lists are independent.
        || id == crate::modules::bio_mcp::bio_mcp_server_id()
        // web_search is approval-bypassed too (read-only search + page fetch,
        // auto-attached); fetched content is treated as untrusted data.
        || id == crate::modules::web_search::web_search_server_id()
        // tool_result is approval-bypassed (read-only recall of the caller's own
        // prior tool results, auto-attached for tool-capable models).
        || id == crate::modules::tool_result_mcp::tool_result_mcp_server_id()
        // lit_search is approval-bypassed (read-only literature search + OA
        // full-text fetch, auto-attached); results are treated as untrusted data.
        || id == crate::modules::lit_search::lit_search_server_id()
}

///
/// Provides Model Context Protocol (MCP) tool calling functionality for chat.
pub struct McpChatExtension {
    pool: PgPool,
    config: Arc<crate::core::config::Config>,
    session_manager: Arc<McpSessionManager>,
    /// Storage for accumulating tool use deltas during streaming
    /// Key: (message_id, content_index)
    tool_use_accumulator: Arc<Mutex<HashMap<(Uuid, usize), AccumulatedToolUse>>>,
}

impl McpChatExtension {
    /// Create new MCP chat extension
    pub fn new(pool: PgPool, config: Arc<crate::core::config::Config>) -> Self {
        let session_manager = Arc::new(McpSessionManager::new(config.clone()));
        Self {
            pool,
            config,
            session_manager,
            tool_use_accumulator: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Execute approved tools and return (MessageContentData results, executed tool_use_ids)
    /// Used by both before_llm_call (no SSE) and after_llm_call (with SSE)
    async fn execute_approved_tools_sync(
        &self,
        approved_pending: &[super::approval::models::ToolUseApproval],
        context: &StreamContext,
        tx: Option<&tokio::sync::mpsc::UnboundedSender<Result<Event, Infallible>>>,
    ) -> Result<(Vec<MessageContentData>, Vec<String>, Option<String>), AppError> {
        let mut tool_results = Vec::new();
        let mut executed_tool_use_ids = Vec::new();
        let accessible_servers =
            helpers::get_all_accessible_config(&self.pool, context.user_id).await?;

        // Channel for elicitation DB persistence (http.rs → mcp.rs via Repos)
        let (elicit_notify_tx, mut elicit_notify_rx) =
            tokio::sync::mpsc::unbounded_channel::<ElicitationStartedNotification>();
        let bind_user_id = context.user_id;
        tokio::spawn(async move {
            while let Some(notif) = elicit_notify_rx.recv().await {
                // Bind the calling user_id to the elicitation entry so
                // the /respond handler can verify the responder is the
                // user who initiated the chat call. Closes
                // 02-permissions F-04.
                crate::modules::mcp::elicitation::registry::bind_owner(
                    notif.elicitation_id,
                    bind_user_id,
                );
                if let Some(msg_id) = notif.message_id {
                    let content_data = MessageContentData::ElicitationRequest {
                        elicitation_id: notif.elicitation_id.to_string(),
                        message: notif.message,
                        requested_schema: notif.requested_schema,
                        server: notif.server,
                        status: "pending".to_string(),
                        response_content: None,
                    };
                    let _ = crate::core::Repos.chat.core
                        .append_content_with_id(notif.content_id, msg_id, "elicitation_request", content_data)
                        .await;
                }
            }
        });

        for approval in approved_pending {
            let tool_use_id = approval.tool_use_id.clone();
            let tool_name = approval.tool_name.clone(); // Clean tool name (e.g., "fetch")
            let input = approval.tool_input.clone();

            // Use server_id from approval record (stored separately)
            let server_id = match approval.server_id {
                Some(id) => id,
                None => {
                    tracing::error!("No server_id in approval record for tool: {}", tool_name);
                    continue;
                }
            };

            // Find server by ID
            let server = accessible_servers.iter().find(|s| s.id == server_id);

            if server.is_none() {
                tracing::error!("Server not found for approved tool: {} (server_id={})", tool_name, server_id);
                let error_result = McpContentData::ToolResult {
                    tool_use_id: tool_use_id.clone(),
                    name: Some(tool_name.clone()),
                    server_id: Some(server_id.to_string()),
                    content: format!("Server '{}' not found", server_id),
                    is_error: Some(true),
                    attachment: None,
                    images: None,
                    resource_links: None,
                    hidden_content: None,
                    structured_content: None,
                };
                tool_results.push(error_result.to_message_content());
                continue;
            }

            let server = server.unwrap();

            // Send tool start event (if tx provided)
            if let Some(tx) = tx {
                helpers::send_tool_start_event(Some(tx), &tool_use_id, &tool_name, &server.name, &input).await;
            }

            // For sampling servers, create a fresh ephemeral session with the LLM handler.
            // Otherwise, use the shared pooled session (existing behaviour).
            let maybe_model_id = context.metadata.get("model_id")
                .and_then(|v| v.as_str())
                .and_then(|s| uuid::Uuid::parse_str(s).ok());

            let mut _owned: Option<McpSession> = None;
            let mut _guard: Option<tokio::sync::OwnedRwLockWriteGuard<McpSession>> = None;

            if server.supports_sampling {
                if let Some(model_id) = maybe_model_id {
                    match ChatSamplingHandler::new(model_id, context.user_id).await {
                        Ok(h) => {
                            let handler = Arc::new(h);
                            match McpSession::new_with_sampling(server.clone(), handler).await {
                                Ok(s) => _owned = Some(s),
                                Err(e) => {
                                    tracing::warn!(
                                        "[sampling] Failed to create sampling session for '{}': {}",
                                        server.name, e
                                    );
                                }
                            }
                        }
                        Err(e) => {
                            tracing::warn!(
                                "[sampling] Failed to init provider for '{}': {}",
                                server.name, e
                            );
                        }
                    }
                } else {
                    tracing::warn!(
                        "[sampling] server '{}' supports_sampling=true but no model_id in context metadata",
                        server.name
                    );
                }
            }

            if _owned.is_none() {
                if server.supports_sampling {
                    // Sampling server but no session could be created (no model_id or provider error).
                    // Fall back to the pooled session would deadlock with SSE-capable servers.
                    tracing::warn!(
                        "[sampling] server '{}' requires sampling but no session could be created; returning error",
                        server.name
                    );
                    let error_result = McpContentData::ToolResult {
                        tool_use_id: tool_use_id.clone(),
                        name: Some(tool_name.to_string()),
                        server_id: Some(server.id.to_string()),
                        content: "Cannot execute sampling tool: no model available. Ensure a model is selected.".to_string(),
                        is_error: Some(true),
                            attachment: None,
                            images: None,
                        resource_links: None,
                        hidden_content: None,
                        structured_content: None,
                    };
                    tool_results.push(error_result.to_message_content());
                    continue;
                }
                let arc = match self.session_manager
                    .get_or_create_with_context(
                        server.id,
                        context.user_id,
                        Some(context.conversation_id),
                        context.message_id,
                    )
                    .await
                {
                    Ok(s) => s,
                    Err(e) => {
                        tracing::warn!(
                            "Failed to get session for MCP server '{}': {}",
                            server.name, e
                        );
                        let err_result = McpContentData::ToolResult {
                            tool_use_id: tool_use_id.clone(),
                            name: Some(tool_name.clone()),
                            server_id: Some(server.id.to_string()),
                            content: format!("Failed to connect to server: {}", e),
                            is_error: Some(true),
                                    attachment: None,
                                    images: None,
                            resource_links: None,
                            hidden_content: None,
                            structured_content: None,
                        };
                        tool_results.push(err_result.to_message_content());
                        continue;
                    }
                };
                _guard = Some(arc.write_owned().await);
            }

            let session: &mut McpSession = if let Some(ref mut s) = _owned {
                s
            } else {
                _guard.as_deref_mut().unwrap()
            };


            // Execute tool with clean tool name
            let (mut result, is_final) = helpers::execute_tool(
                session,
                &tool_name,
                input,
                &server.name,
                Some(server.timeout_seconds),
                context.message_id,
                tx.cloned(),
                Some(elicit_notify_tx.clone()),
            )
            .await;

            // Set tool_use_id and server_id
            if let McpContentData::ToolResult {
                tool_use_id: ref mut id,
                server_id: ref mut sid,
                is_error,
                ref content,
                ..
            } = result
            {
                *id = tool_use_id.clone();
                *sid = Some(server.id.to_string());

                // Send tool complete event (if tx provided)
                if let Some(tx) = tx {
                    helpers::send_tool_complete_event(
                        Some(tx),
                        &tool_use_id,
                        &tool_name,
                        &server.name,
                        is_error.unwrap_or(false),
                        Some(content.as_str()),
                    )
                    .await;
                }
            }

            // Generic resource_link handling: fetch-and-save any resource_links returned by a tool.
            // Works uniformly for built-in servers (short-lived JWT auth) and external MCP servers
            // (server-configured headers). Runs the full processing pipeline (text extraction,
            // thumbnails) and creates a permanent DB artifact visible to the user.
            // Exception: is_saved=true links already exist in originals storage — skip all processing.
            let mut saved_artifacts: Vec<(Uuid, String, Option<String>)> = Vec::new(); // (artifact_id, display_name, download_url)
            let mut saved_file_urls: Vec<(String, String)> = Vec::new(); // (display_name, download_url) for is_saved links
            // (link_index, artifact_id) for workspace artifacts saved by this
            // pipeline. Applied back onto resource_links[i].file_id after the loop
            // so the browser inline preview can fetch via the authenticated,
            // same-origin /api/files/{id}/... path (the tool-emitted absolute
            // loopback URI is unreachable from the browser).
            // (link_idx, file_id, version, version_id) — version/version_id pin
            // the inline preview's resource_link to the exact artifact version.
            let mut artifact_file_ids: Vec<(usize, Uuid, i32, Uuid)> = Vec::new();
            if let McpContentData::ToolResult { ref resource_links, is_error, .. } = result
                && !is_error.unwrap_or(false)
                    && let Some(links) = resource_links {
                        for (link_idx, link) in links.iter().enumerate() {

                        // is_saved=true: file already exists in originals storage.
                        // URI is a download-with-token URL — skip fetch/process/save pipeline.
                        if link.is_saved == Some(true) {
                            let name = link.name.as_deref().unwrap_or("file").to_string();
                            saved_file_urls.push((name, link.uri.clone()));
                            continue;
                        }

                        use crate::modules::file::models::FileCreateData;
                        use crate::modules::file::processing::ProcessingManager;
                        use crate::modules::file::storage::manager::get_file_storage;

                        // Build auth headers appropriate for the server type
                        let mut fetch_headers = reqwest::header::HeaderMap::new();
                        if server.is_built_in {
                            match McpSessionManager::generate_short_lived_jwt(
                                context.user_id, &self.config.jwt.secret, 10
                            ) {
                                Ok(token) => {
                                    if let Ok(hval) = reqwest::header::HeaderValue::from_str(
                                        &format!("Bearer {}", token)
                                    ) {
                                        fetch_headers.insert(reqwest::header::AUTHORIZATION, hval);
                                    }
                                    if let Ok(hval) = reqwest::header::HeaderValue::from_str(
                                        &context.conversation_id.to_string()
                                    ) {
                                        fetch_headers.insert(
                                            reqwest::header::HeaderName::from_static("x-conversation-id"),
                                            hval,
                                        );
                                    }
                                }
                                Err(e) => {
                                    tracing::warn!("Failed to generate JWT for resource_link fetch: {}", e);
                                }
                            }
                        } else if let Some(headers_map) = server.headers.as_object() {
                            for (key, value) in headers_map.iter() {
                                if let Some(val_str) = value.as_str()
                                    && let (Ok(hname), Ok(hval)) = (
                                        reqwest::header::HeaderName::from_bytes(key.as_bytes()),
                                        reqwest::header::HeaderValue::from_str(val_str),
                                    ) {
                                        fetch_headers.insert(hname, hval);
                                    }
                            }
                        }

                        match reqwest::Client::builder()
                            .default_headers(fetch_headers)
                            .build()
                        {
                            Ok(client) => {
                                match client.get(&link.uri).send().await {
                                    Ok(response) if response.status().is_success() => {
                                        let content_type_mime = response
                                            .headers()
                                            .get(reqwest::header::CONTENT_TYPE)
                                            .and_then(|v| v.to_str().ok())
                                            .and_then(|s| s.split(';').next())
                                            .map(|s| s.trim().to_string());

                                        match response.bytes().await {
                                            Ok(bytes) => {
                                                let bytes = bytes.to_vec();
                                                let display_name =
                                                    link.name.as_deref().unwrap_or("file");
                                                // Canonical extension (rsplit + lowercase) — MUST match how
                                                // the download/read paths derive the blob key. Path::extension
                                                // would save dotfiles / no-extension names (`.bashrc`,
                                                // `Makefile`) as `…​.bin` but load them as `…​.bashrc` → 404.
                                                let ext =
                                                    crate::modules::file::utils::extension_of(display_name);
                                                let mime_type = content_type_mime.or_else(|| {
                                                    mime_guess::from_ext(&ext)
                                                        .first()
                                                        .map(|m| m.to_string())
                                                });
                                                let mime_type_str = mime_type
                                                    .as_deref()
                                                    .unwrap_or("application/octet-stream");

                                                let processing_result = ProcessingManager::new()
                                                    .process_file(&bytes, mime_type_str)
                                                    .await
                                                    .unwrap_or_default();

                                                let artifact_id = Uuid::new_v4();
                                                let storage = get_file_storage();

                                                match storage
                                                    .save_original(
                                                        context.user_id,
                                                        artifact_id,
                                                        &ext,
                                                        &bytes,
                                                    )
                                                    .await
                                                {
                                                    Ok(_) => {
                                                        for (n, text) in processing_result
                                                            .text_pages
                                                            .iter()
                                                            .enumerate()
                                                        {
                                                            let _ = storage
                                                                .save_text_page(
                                                                    context.user_id,
                                                                    artifact_id,
                                                                    (n + 1) as u32,
                                                                    text,
                                                                )
                                                                .await;
                                                        }
                                                        if let Some(thumb) = processing_result
                                                            .thumbnails
                                                            .first()
                                                        {
                                                            let _ = storage
                                                                .save_image(
                                                                    context.user_id,
                                                                    artifact_id,
                                                                    1,
                                                                    true,
                                                                    thumb,
                                                                )
                                                                .await;
                                                        }
                                                        for (n, img) in processing_result
                                                            .images
                                                            .iter()
                                                            .enumerate()
                                                        {
                                                            let _ = storage
                                                                .save_image(
                                                                    context.user_id,
                                                                    artifact_id,
                                                                    (n + 1) as u32,
                                                                    false,
                                                                    img,
                                                                )
                                                                .await;
                                                        }

                                                        let file_size = bytes.len() as i64;
                                                        // Real checksum: version-back's no-op check compares the
                                                        // workspace bytes' checksum to the base version's. A `None`
                                                        // base never matches → every staged artifact would spuriously
                                                        // version-back even when unchanged.
                                                        let checksum =
                                                            storage.calculate_checksum(&bytes);
                                                        match Repos
                                                            .file
                                                            .create(FileCreateData {
                                                                id: artifact_id,
                                                                user_id: context.user_id,
                                                                filename: display_name
                                                                    .to_string(),
                                                                file_size,
                                                                mime_type: mime_type.clone(),
                                                                checksum: Some(checksum),
                                                                has_thumbnail:
                                                                    !processing_result
                                                                        .thumbnails
                                                                        .is_empty(),
                                                                preview_page_count:
                                                                    processing_result
                                                                        .images
                                                                        .len()
                                                                        as i32,
                                                                text_page_count:
                                                                    processing_result
                                                                        .text_pages
                                                                        .len()
                                                                        as i32,
                                                                processing_metadata:
                                                                    serde_json::to_value(
                                                                        &processing_result
                                                                            .metadata,
                                                                    )
                                                                    .unwrap_or_default(),
                                                                source_message_id:
                                                                    context.message_id,
                                                                created_by: "mcp".to_string(),
                                                            })
                                                            .await
                                                        {
                                                            Ok(file) => {
                                                                helpers::send_artifact_created_event(
                                                                    tx,
                                                                    &tool_use_id,
                                                                    &artifact_id.to_string(),
                                                                    display_name,
                                                                    mime_type.as_deref(),
                                                                    file_size,
                                                                )
                                                                .await;

                                                                // Notify the user's OTHER devices a new file exists
                                                                // (cross-device sync), mirroring files_mcp's create
                                                                // path — send_artifact_created_event above only reaches
                                                                // THIS conversation's SSE stream.
                                                                crate::modules::file::sync::publish_file_changed(
                                                                    context.user_id,
                                                                    artifact_id,
                                                                );

                                                                // No FileAttachment block is emitted for artifacts: the
                                                                // inline preview (resource_link, stamped after the loop)
                                                                // is the single UI view. Record index→(file_id, version,
                                                                // version_id) so it can pin the exact version created here.
                                                                artifact_file_ids.push((
                                                                    link_idx,
                                                                    artifact_id,
                                                                    file.version,
                                                                    file.current_version_id,
                                                                ));

                                                                tracing::info!(
                                                                    "Artifact saved from resource_link: file_id={}, filename={}",
                                                                    artifact_id, display_name
                                                                );
                                                                let download_url = {
                                                                    use jsonwebtoken::{encode, EncodingKey, Header as JwtHeader};
                                                                    use crate::modules::file::types::{DownloadTokenClaims, DOWNLOAD_TOKEN_AUDIENCE};
                                                                    let now = chrono::Utc::now().timestamp() as usize;
                                                                    let claims = DownloadTokenClaims {
                                                                        file_id: artifact_id.to_string(),
                                                                        user_id: context.user_id.to_string(),
                                                                        version: None,
                                                                        exp: now + 3600,
                                                                        iat: now,
                                                                        iss: self.config.jwt.issuer.clone(),
                                                                        aud: DOWNLOAD_TOKEN_AUDIENCE.to_string(),
                                                                    };
                                                                    // Root the tool-to-tool download URL at the SAME origin
                                                                    // get_resource_link uses (public_base_url when set, else the
                                                                    // pinned 127.0.0.1 loopback) — NOT self.config.server.host,
                                                                    // which may be 0.0.0.0 / a bind address unreachable by the
                                                                    // (possibly remote) MCP server the LLM passes this URL to.
                                                                    let origin = file_download_origin(
                                                                        self.config.code_sandbox.as_ref(),
                                                                        self.config.server.port,
                                                                    );
                                                                    encode(
                                                                        &JwtHeader::default(),
                                                                        &claims,
                                                                        &EncodingKey::from_secret(self.config.jwt.secret.as_bytes()),
                                                                    )
                                                                    .ok()
                                                                    .map(|token| build_artifact_download_url(
                                                                        &origin,
                                                                        &self.config.server.api_prefix,
                                                                        artifact_id,
                                                                        &token,
                                                                    ))
                                                                };
                                                                saved_artifacts.push((artifact_id, display_name.to_string(), download_url));
                                                            }
                                                            Err(e) => {
                                                                tracing::error!(
                                                                    "Failed to create file DB record for resource_link: {}",
                                                                    e
                                                                );
                                                            }
                                                        }
                                                    }
                                                    Err(e) => {
                                                        tracing::error!(
                                                            "Failed to save artifact original: {}",
                                                            e
                                                        );
                                                    }
                                                }
                                            }
                                            Err(e) => {
                                                tracing::warn!(
                                                    "Failed to read resource_link response body: {}",
                                                    e
                                                );
                                            }
                                        }
                                    }
                                    Ok(response) => {
                                        tracing::error!(
                                            "resource_link fetch returned HTTP {} for '{}': artifact NOT saved",
                                            response.status(),
                                            link.uri
                                        );
                                    }
                                    Err(e) => {
                                        tracing::error!(
                                            "Failed to fetch resource_link '{}': {} — artifact NOT saved",
                                            link.uri, e
                                        );
                                    }
                                }
                            }
                            Err(e) => {
                                tracing::error!(
                                    "Failed to build HTTP client for resource_link fetch: {}",
                                    e
                                );
                            }
                        }
                        } // end for link in links
                    }

            // Update tool result content with the saved artifact info so the LLM knows the file_ids.
            // Also set hidden_content with token-based download URLs — included in LLM messages
            // but stripped from browser API responses.
            // saved_file_urls holds download-with-token URLs for is_saved=true links (no pipeline needed).
            if (!saved_artifacts.is_empty() || !saved_file_urls.is_empty())
                && let McpContentData::ToolResult { ref mut content, ref mut hidden_content, ref mut resource_links, .. } = result {
                    // Stamp each saved artifact's file_id onto its resource_link so
                    // the UI inline preview fetches the content via the authenticated
                    // /api/files/{id}/... path instead of the unreachable absolute
                    // loopback URI emitted by the tool.
                    if let Some(links) = resource_links {
                        for (idx, fid, ver, ver_id) in &artifact_file_ids {
                            if let Some(l) = links.get_mut(*idx) {
                                l.file_id = Some(*fid);
                                l.version = Some(*ver);
                                l.version_id = Some(*ver_id);
                            }
                        }
                    }
                    if !saved_artifacts.is_empty() {
                        let file_descriptions: Vec<String> = saved_artifacts
                            .iter()
                            .map(|(id, name, _)| format!("'{}' (file_id: {})", name, id))
                            .collect();
                        *content = format!(
                            "Files from MCP tool have been saved as artifact attachments: {}. \
                             They will be shown as inline file previews in the UI — do not embed them inline in your response.",
                            file_descriptions.join(", ")
                        );
                    }
                    let mut url_lines: Vec<String> = saved_artifacts
                        .iter()
                        .filter_map(|(_, name, url)| url.as_ref().map(|u| format!("{} - {}", name, u)))
                        .collect();
                    for (name, url) in &saved_file_urls {
                        url_lines.push(format!("{} - {}", name, url));
                    }
                    if !url_lines.is_empty() {
                        *hidden_content = Some(format!(
                            "[system: Files saved as artifact attachments (shown as file cards in UI). \
                             Do NOT embed file URLs or images inline in your text response. \
                             To pass one of these files to another tool, copy its URL below \
                             VERBATIM into that tool's file/URL argument — never rewrite the host, \
                             never substitute 127.0.0.1/localhost, and never invent a DRS or \
                             platform URL. The URLs below are already reachable exactly as given \
                             (do not call get_resource_link for these — use the URL here directly):\n{}]",
                            url_lines.join("\n")
                        ));
                    }
                }

            // Track executed tool_use_id
            executed_tool_use_ids.push(tool_use_id.clone());

            // Delete approval record after successful execution to prevent double-execution
            if let Err(e) = Repos
                .chat
                .mcp
                .delete_tool_approval(tool_use_id.clone(), approval.message_id)
                .await
            {
                tracing::error!(
                    "Failed to delete approval record for tool_use_id={}: {}. This may cause duplicate execution attempts.",
                    tool_use_id,
                    e
                );
            }

            // If this tool returns a final response, capture it and return early.
            // The caller will stream it directly without calling the LLM.
            if is_final
                && let McpContentData::ToolResult { ref content, .. } = result {
                    tracing::info!(
                        "audience=[\"user\"]: approved tool '{}' marked as final, will bypass LLM",
                        tool_name
                    );
                    let final_response = Some(content.clone());
                    // Push the tool_result BEFORE returning so the caller can persist it to DB.
                    // Without this, the tool_use in the assistant message would have no matching
                    // tool_result, causing "tool_use ids found without tool_result" on the next message.
                    tool_results.push(result.to_message_content());
                    return Ok((tool_results, executed_tool_use_ids, final_response));
                }

            // Convert to MessageContentData and add to results
            tool_results.push(result.to_message_content());
        }

        Ok((tool_results, executed_tool_use_ids, None))
    }
}

#[async_trait]
impl ChatExtension for McpChatExtension {
    fn name(&self) -> &str {
        "mcp"
    }

    /// Don't create user message if we're resuming with tool approvals
    /// Tool approval resumption continues the existing conversation turn
    fn should_create_user_message(&self, request: &SendMessageRequest) -> bool {
        request.tool_approvals.is_none()
    }

    /// Provide existing assistant message when resuming with tool approvals
    /// Tool results append to the existing assistant message, not a new one
    async fn provide_assistant_message(
        &self,
        request: &SendMessageRequest,
        branch_id: Uuid,
    ) -> Result<Option<Uuid>, AppError> {
        // Only provide message if resuming with tool approvals
        if request.tool_approvals.is_some() {
            // Get last assistant message in branch
            let history = Repos.chat.core.get_conversation_history(branch_id).await?;

            // Find last assistant message
            let last_assistant = history.iter()
                .rev()
                .find(|msg| msg.message.role == "assistant");

            if let Some(msg) = last_assistant {
                return Ok(Some(msg.message.id));
            }
        }

        Ok(None)
    }

    /// Convert MCP content (ToolUse, ToolResult) to ContentBlock for LLM
    async fn process_content_for_llm(
        &self,
        content: &MessageContentData,
        _context: &StreamContext,
    ) -> Result<Option<ContentBlock>, AppError> {
        // Try to convert MessageContentData to McpContentData
        if let Ok(mcp_content) = McpContentData::from_message_content(content) {
            // Convert to ContentBlock (handles both ToolUse and ToolResult)
            Ok(mcp_content.to_content_block())
        } else {
            Ok(None) // Not MCP content
        }
    }

    /// Register MCP bridge routes (approval + per-user defaults).
    ///
    /// Both routers register through the ChatExtension trait so chat
    /// doesn't have to know they exist. Previously `mcp_defaults_router`
    /// was merged explicitly in `chat/mod.rs::register_routes`; that
    /// direct chat→mcp wire-up went away with the bridge extraction.
    fn register_routes(&self, router: ApiRouter) -> ApiRouter {
        router
            .merge(super::approval::mcp_approval_router())
            .merge(super::defaults::mcp_defaults_router())
            // GET /api/messages/{id}/mcp-servers — the per-message
            // server-list snapshot that replaced
            // `messages.mcp_server_ids` after migration 74. Owned by
            // the mcp bridge, not chat.
            .merge(super::message_servers_routes::message_mcp_servers_router())
    }

    /// Snapshot the MCP servers enabled at user-message-send time into
    /// the `message_mcp_servers` join table. Used by the frontend mcp
    /// extension on Edit to restore the original server selection.
    ///
    /// Replaces the pre-extraction pattern where chat's `messages`
    /// table owned a `mcp_server_ids UUID[]` column populated inline
    /// by `streaming.rs`. After migration 74, that column is gone and
    /// this hook is the sole writer.
    ///
    /// Soft-fail behavior: if the INSERT fails (e.g. DB blip), the
    /// message is already saved without server tracking. Edit-restore
    /// then degrades to "use current MCP selection" — same fallback as
    /// messages from before the column was added.
    async fn after_user_message_created(
        &self,
        _context: &StreamContext,
        user_message: &Message,
        send_request: &SendMessageRequest,
    ) -> Result<(), AppError> {
        let Some(config) = &send_request.mcp_config else {
            return Ok(());
        };
        let server_ids: Vec<Uuid> = config
            .mcp_servers
            .iter()
            .map(|s| s.server_id)
            .collect();
        if server_ids.is_empty() {
            return Ok(());
        }
        Repos
            .chat
            .mcp
            .insert_message_servers(user_message.id, &server_ids)
            .await
    }

    async fn before_llm_call(
        &self,
        context: &mut StreamContext,
        request: &mut ChatRequest,
        send_request: &SendMessageRequest,
        tx: Option<&tokio::sync::mpsc::UnboundedSender<Result<axum::response::sse::Event, std::convert::Infallible>>>,
    ) -> Result<BeforeLlmAction, AppError> {
        // === STEP 1: Process tool approvals (if resuming after approval) ===
        if let Some(approvals) = &send_request.tool_approvals {
            tracing::info!(
                "Processing {} tool approval decisions for conversation {}, branch {}",
                approvals.len(),
                context.conversation_id,
                context.branch_id
            );

            // Log each approval decision for debugging
            for (idx, approval) in approvals.iter().enumerate() {
                tracing::info!(
                    "Approval[{}]: tool_use_id='{}', decision='{}', note={:?}",
                    idx,
                    approval.tool_use_id,
                    approval.decision,
                    approval.note
                );
            }

            // Process each approval decision
            for approval in approvals {
                tracing::info!("Processing approval decision: tool_use_id={}, decision={}, branch_id={}",
                    approval.tool_use_id, approval.decision, context.branch_id);
                match approval.decision.as_str() {
                    "approve" | "approved" => {
                        // Check what pending approvals exist for this branch
                        let pending = super::approval::repository::get_pending_approvals_for_branch(
                            &self.pool,
                            context.branch_id,
                        )
                        .await?;
                        tracing::info!(
                            "Pending approvals for branch {}: {:?}",
                            context.branch_id,
                            pending.iter().map(|p| (&p.tool_use_id, &p.status)).collect::<Vec<_>>()
                        );

                        // Check if this tool_use_id is still pending (idempotency check)
                        let is_pending = pending.iter().any(|p| p.tool_use_id == approval.tool_use_id);
                        if !is_pending {
                            tracing::info!(
                                "Approval for tool_use_id={} already processed (not in pending list), skipping",
                                approval.tool_use_id
                            );
                            continue;
                        }

                        // Approve the tool use
                        tracing::info!("Calling approve_tool_use for tool_use_id={}, branch_id={}",
                            approval.tool_use_id, context.branch_id);
                        match super::approval::repository::approve_tool_use(
                            &self.pool,
                            approval.tool_use_id.clone(),
                            context.branch_id,
                            context.user_id,
                            approval.note.clone(),
                        )
                        .await {
                            Ok(approval_record) => {
                                tracing::info!("Successfully approved tool use: tool_use_id={}, status={}, branch_id={}, approval_id={}",
                                    approval.tool_use_id, approval_record.status, approval_record.branch_id, approval_record.id);
                            }
                            Err(e) => {
                                // Handle "not found" gracefully - might be a retry of an already-processed approval
                                if e.to_string().contains("not found") || e.to_string().contains("already processed") {
                                    tracing::warn!(
                                        "Approval for tool_use_id={} was already processed (concurrent request?), continuing",
                                        approval.tool_use_id
                                    );
                                    continue;
                                }
                                tracing::error!("Failed to approve tool use {}: {}", approval.tool_use_id, e);
                                return Err(e);
                            }
                        }
                    }
                    "deny" | "denied" => {
                        // Deny the tool use (with idempotency handling)
                        match super::approval::repository::deny_tool_use(
                            &self.pool,
                            approval.tool_use_id.clone(),
                            context.branch_id,
                            context.user_id,
                            approval.note.clone(),
                        )
                        .await {
                            Ok(_) => {
                                tracing::info!("Denied tool use: {}", approval.tool_use_id);
                            }
                            Err(e) => {
                                // Handle "not found" gracefully - might be a retry of an already-processed denial
                                if e.to_string().contains("not found") || e.to_string().contains("already processed") {
                                    tracing::warn!(
                                        "Denial for tool_use_id={} was already processed (concurrent request?), continuing",
                                        approval.tool_use_id
                                    );
                                    continue;
                                }
                                tracing::error!("Failed to deny tool use {}: {}", approval.tool_use_id, e);
                                return Err(e);
                            }
                        }
                    }
                    _ => {
                        return Err(AppError::bad_request(
                            "INVALID_DECISION",
                            format!("Invalid decision: '{}'. Must be 'approve'/'approved' or 'deny'/'denied'", approval.decision),
                        ));
                    }
                }
            }

            // === STEP 1b: Check if all tools were denied ===
            // If all approvals were denied, skip LLM call and complete gracefully
            let all_denied = approvals.iter().all(|a|
                a.decision == "deny" || a.decision == "denied"
            );

            if all_denied {
                tracing::info!("All {} tool approvals were denied, skipping LLM call", approvals.len());

                // (Previously emitted a best-effort `tool_denied` SSE event the
                // client never handled; dropped with the move to the typed
                // chat-token channel — the turn just completes.)

                return Ok(BeforeLlmAction::Complete);
            }

            // === STEP 1b.5: Guard — don't proceed if other tool_uses are still awaiting a decision ===
            // When the LLM requested multiple parallel tool calls that all need approval and the
            // user approves them one at a time, we must wait until every tool_use has been either
            // approved or denied before executing anything or calling the LLM.  Otherwise the LLM
            // request would contain tool_use blocks without matching tool_result blocks, causing
            // "tool_use ids found without tool_result" errors.
            let still_pending = super::approval::repository::get_pending_approvals_for_branch(
                &self.pool,
                context.branch_id,
            )
            .await?;

            if !still_pending.is_empty() {
                tracing::info!(
                    "{} pending approval(s) still remain after processing {} decision(s); \
                     waiting for remaining approvals before executing tools or calling LLM",
                    still_pending.len(),
                    approvals.len()
                );
                return Ok(BeforeLlmAction::Complete);
            }

            // === STEP 1c: Execute approved tools immediately after approval ===
            let approved_pending = super::approval::repository::get_approved_tools_for_branch(
                &self.pool,
                context.branch_id,
            )
            .await?;

            tracing::info!("before_llm_call: Found {} approved tools after processing approvals", approved_pending.len());

            // Collect all content blocks from both approved and denied tools so they can be
            // sent as a single User message.  Anthropic requires that every tool_use block in
            // the preceding assistant turn has a matching tool_result block in the next user
            // turn; mixing approved and denied results in one message satisfies that constraint.
            let mut content_blocks: Vec<ai_providers::ContentBlock> = Vec::new();

            if !approved_pending.is_empty() {
                // Execute approved tools and append results to request
                let (tool_results, executed_ids, final_response) = self.execute_approved_tools_sync(
                    &approved_pending,
                    context,
                    tx,
                ).await?;

                // Save tool results to the assistant message in database BEFORE any early returns.
                // This ensures tool_result blocks are persisted even when audience=["user"] bypasses the LLM
                // bypasses the normal Continue action. Without this, the tool_use block already in
                // the DB would have no matching tool_result, causing API errors on subsequent messages.
                if let Some(message_id) = context.message_id {
                    // append_content assigns sequence_order atomically (MAX+1), so these
                    // results can't collide with the tool_use blocks finalize() wrote nor
                    // with a concurrent iteration's blocks.
                    for result in tool_results.iter() {
                        let content_type = result.content_type();

                        match Repos.chat.core.append_content(
                            message_id,
                            &content_type,
                            result.clone(),
                        ).await {
                            Ok(created) => tracing::info!(
                                "Saved tool_result to message {}, sequence {}",
                                message_id, created.sequence_order
                            ),
                            Err(e) => tracing::error!("Failed to save tool result to message: {}", e),
                        }
                    }

                    // Cancel any elicitations that are still pending after tool execution ends
                    // (e.g., tool timed out while waiting for user input).
                    let _ = Repos.chat.core.cancel_pending_elicitations(message_id).await;
                }

                // If any approved tool emitted audience=["user"] content, bypass LLM entirely.
                // tool_results are already saved to DB above.
                if let Some(text) = final_response {
                    return Ok(BeforeLlmAction::CompleteWithContent { text });
                }

                // Store executed tool_use_ids in context metadata for later filtering
                if !executed_ids.is_empty() {
                    // Merge with any existing executed IDs
                    let mut all_executed: Vec<String> = context.metadata
                        .get("executed_tool_use_ids")
                        .and_then(|v| serde_json::from_value(v.clone()).ok())
                        .unwrap_or_default();
                    all_executed.extend(executed_ids.clone());
                    context.metadata.insert(
                        "executed_tool_use_ids".to_string(),
                        serde_json::to_value(&all_executed).unwrap_or_default(),
                    );
                    tracing::info!(
                        "Tracked {} executed tool_use_ids in context: {:?}",
                        executed_ids.len(),
                        executed_ids
                    );
                }

                // Convert approved tool results to content blocks
                for result in tool_results {
                    if let Some(block) = self.process_content_for_llm(&result, context).await? {
                        content_blocks.push(block);
                    }
                }
            }

            // === STEP 1d: Generate error tool_results for denied tools ===
            // Denied tools have no real result, but the LLM requires a tool_result for every
            // tool_use it emitted.  We create a synthetic error result so the message history
            // remains valid, then delete the denial record to prevent re-processing.
            let denied_tools = super::approval::repository::get_denied_tools_for_branch(
                &self.pool,
                context.branch_id,
            )
            .await?;

            if !denied_tools.is_empty() {
                tracing::info!(
                    "before_llm_call: Creating error tool_results for {} denied tool(s)",
                    denied_tools.len()
                );

                if let Some(message_id) = context.message_id {
                    for denied in denied_tools.iter() {
                        let denied_result = McpContentData::ToolResult {
                            tool_use_id: denied.tool_use_id.clone(),
                            name: Some(denied.tool_name.clone()),
                            server_id: denied.server_id.map(|id| id.to_string()),
                            content: "Tool execution was denied by the user.".to_string(),
                            is_error: Some(true),
                                    attachment: None,
                                    images: None,
                            resource_links: None,
                            hidden_content: None,
                            structured_content: None,
                        };
                        let msg_content = denied_result.to_message_content();

                        // Persist denied result so the conversation history stays coherent.
                        // append_content assigns sequence_order atomically (MAX+1).
                        let content_type = msg_content.content_type();
                        if let Err(e) = Repos.chat.core.append_content(
                            message_id,
                            &content_type,
                            msg_content.clone(),
                        ).await {
                            tracing::error!(
                                "Failed to save denied tool_result for tool_use_id={}: {}",
                                denied.tool_use_id, e
                            );
                        } else {
                            tracing::info!(
                                "Saved denied tool_result for tool_use_id={} to message {}",
                                denied.tool_use_id, message_id
                            );
                        }

                        // Convert for LLM request
                        if let Some(block) = self.process_content_for_llm(&msg_content, context).await? {
                            content_blocks.push(block);
                        }

                        // Delete the denial record so it isn't processed again on future resumptions
                        if let Err(e) = Repos.chat.mcp
                            .delete_tool_approval(denied.tool_use_id.clone(), denied.message_id)
                            .await
                        {
                            tracing::error!(
                                "Failed to delete denial record for tool_use_id={}: {}",
                                denied.tool_use_id, e
                            );
                        }
                    }
                }
            }

            // Append all tool results (approved + denied) as a single user message
            if !content_blocks.is_empty() {
                use ai_providers::{ChatMessage, Role};
                let count = content_blocks.len();
                request.messages.push(ChatMessage {
                    role: Role::User,
                    content: content_blocks,
                });
                tracing::info!("Appended {} tool result(s) to request (approved + denied)", count);
            }
        } else {
            // No tool_approvals provided - check if there are pending approvals to cancel
            let pending_count = super::approval::repository::get_pending_approvals_for_branch(
                &self.pool,
                context.branch_id,
            )
            .await?
            .len();

            if pending_count > 0 {
                tracing::info!(
                    "Cancelling {} pending approvals for branch {} (new message without approval)",
                    pending_count,
                    context.branch_id
                );
                super::approval::repository::cancel_pending_approvals_for_branch(
                    &self.pool,
                    context.branch_id,
                )
                .await?;
            }
        }

        // === STEP 2: Check if MCP is enabled ===
        // Built-in servers (files = Track A, memory = Track B inline self-save)
        // auto-attach whenever the file/memory extensions flagged them — even
        // when general MCP is off, so a user with MCP disabled still gets agentic
        // file reading + memory saving.
        let builtin_ids = auto_attach_builtin_ids(&context.metadata);
        if !send_request.enable_mcp && builtin_ids.is_empty() {
            tracing::debug!("MCP not enabled for this request");
            return Ok(BeforeLlmAction::Continue);
        }

        // Get mcp_servers from config (only when general MCP is enabled — when
        // ONLY built-in servers are auto-attaching, we attach just those).
        // NOTE: the disabled path requests an explicit EMPTY list, NOT None.
        // `validate_and_build_config(.., None)` means "no specific servers
        // requested → use ALL accessible servers", which would inject (and
        // pre-execute, for Always-mode servers) every third-party MCP server
        // the user can access despite MCP being turned off. `Some(vec![])`
        // produces an empty config so only the appended built-ins survive.
        let mcp_servers = if send_request.enable_mcp {
            send_request
                .mcp_config
                .as_ref()
                .map(|config| config.mcp_servers.clone())
        } else {
            Some(Vec::new())
        };

        tracing::info!(
            "MCP extension: enabled for user {}, servers requested: {}",
            context.user_id,
            mcp_servers.as_ref().map(|s| s.len()).unwrap_or(0)
        );

        // Validate and build server configuration
        let (mut server_configs, accessible_ids) =
            helpers::validate_and_build_config(&self.pool, context.user_id, mcp_servers).await?;

        // Fetch the auto-attached built-ins by deterministic id, OUTSIDE the
        // group-gated accessibility path (they have no user_group grant). Empty
        // tool list = all of their tools.
        let mut builtin_servers: Vec<crate::modules::mcp::models::McpServer> = Vec::new();
        for id in &builtin_ids {
            // `get_any_server` ignores `enabled`; guard it here so a built-in an
            // operator/health-check disabled is not auto-attached (and approval-
            // bypassed). No shipping path disables a built-in today, so this is
            // defense-in-depth.
            if let Some(s) = crate::core::Repos.mcp.get_any_server(*id).await? {
                if s.enabled {
                    builtin_servers.push(s);
                }
            }
        }
        for s in &builtin_servers {
            if !server_configs.iter().any(|(id, _)| id == &s.id) {
                server_configs.push((s.id, Vec::new()));
            }
        }

        if server_configs.is_empty() {
            tracing::debug!(
                "User {} can access 0 MCP servers (out of {} accessible)",
                context.user_id,
                accessible_ids.len()
            );
            return Ok(BeforeLlmAction::Continue);
        }

        // Get all accessible servers with details (+ the auto-attached built-ins
        // so the tool-listing loop can resolve their details).
        let mut accessible_servers =
            helpers::get_all_accessible_config(&self.pool, context.user_id).await?;
        for s in builtin_servers {
            if !accessible_servers.iter().any(|x| x.id == s.id) {
                accessible_servers.push(s);
            }
        }

        // Extract user's raw message text (used for "always"-mode preprocessing)
        let user_message_text: Option<String> = request.messages.iter().rev()
            .find(|m| m.role == ai_providers::Role::User)
            .and_then(|m| m.content.iter().find_map(|block| {
                if let ai_providers::ContentBlock::Text { text } = block {
                    Some(text.clone())
                } else {
                    None
                }
            }));

        // Collect tools from all configured servers
        let mut all_tools = Vec::new();
        let mut always_mode_context: Vec<String> = Vec::new();

        for (server_id, requested_tools) in &server_configs {
            // Find server details
            let server = accessible_servers
                .iter()
                .find(|s| s.id == *server_id)
                .ok_or_else(|| AppError::internal_error("Server not found in accessible list"))?;

            if server.usage_mode == UsageMode::Always {
                // Always mode: pre-run tools with user's message and inject enriched context
                if let Some(ref query_text) = user_message_text {
                    let maybe_model_id = context.metadata.get("model_id")
                        .and_then(|v| v.as_str())
                        .and_then(|s| uuid::Uuid::parse_str(s).ok());

                    // Create session (with sampling if supported)
                    let session_result = if server.supports_sampling {
                        if let Some(model_id) = maybe_model_id {
                            match ChatSamplingHandler::new(model_id, context.user_id).await {
                                Ok(h) => McpSession::new_with_sampling(server.clone(), Arc::new(h)).await,
                                Err(e) => {
                                    tracing::warn!("Always-mode: failed to init sampling provider for {}: {}", server.name, e);
                                    McpSession::new(server.clone()).await
                                }
                            }
                        } else {
                            McpSession::new(server.clone()).await
                        }
                    } else {
                        McpSession::new(server.clone()).await
                    };

                    match session_result {
                        Err(e) => {
                            tracing::warn!("Always-mode: failed to connect to server {}: {}", server.name, e);
                        }
                        Ok(mut session) => {
                            let mcp_tools = match session.list_tools().await {
                                Ok(t) => t,
                                Err(e) => {
                                    tracing::warn!("Always-mode: failed to list tools from {}: {}", server.name, e);
                                    Vec::new()
                                }
                            };

                            let tools_to_run: Vec<_> = if requested_tools.is_empty() {
                                mcp_tools
                            } else {
                                mcp_tools.into_iter().filter(|t| requested_tools.contains(&t.name)).collect()
                            };

                            for tool in &tools_to_run {
                                // build_query_input returns None when the schema has required
                                // non-string params — skip auto-execution rather than submitting
                                // wrong inputs silently.
                                let input = match helpers::build_query_input(&tool.input_schema, query_text) {
                                    Some(v) => v,
                                    None => {
                                        tracing::debug!(
                                            "[mcp] Skipping always-mode tool '{}': schema has required non-string params",
                                            tool.name
                                        );
                                        continue;
                                    }
                                };
                                match session.call_tool(&tool.name, input, context.message_id, None, None).await {
                                    Ok(result) => {
                                        // Collect text content from tool result
                                        let text_parts: Vec<String> = result.content.iter()
                                            .filter_map(|c| c.content.get("text").and_then(|t| t.as_str()).map(|s| s.to_string()))
                                            .collect();
                                        if !text_parts.is_empty() {
                                            always_mode_context.push(format!(
                                                "[{}] {}:\n{}",
                                                server.name,
                                                tool.name,
                                                text_parts.join("\n")
                                            ));
                                        }
                                    }
                                    Err(e) => {
                                        tracing::warn!("Always-mode: tool {} on {} failed: {}", tool.name, server.name, e);
                                    }
                                }
                            }
                        }
                    }
                }
                continue; // Don't add "always" server tools to the LLM tool list
            }

            // `ask_user` is intercepted in execute_tool and NEVER dispatched over
            // the loopback, so advertise its STATIC descriptor directly instead of
            // paying a loopback initialize + tools/list round-trip on every
            // tool-capable turn. The wire name (`<server_id>__ask_user`) is
            // identical to what list_tools would have produced.
            if *server_id == crate::modules::elicitation_mcp::elicitation_mcp_server_id() {
                let list = crate::modules::elicitation_mcp::tools::tool_list();
                if let Some(arr) = list["tools"].as_array() {
                    for t in arr {
                        let mcp_tool = crate::modules::mcp::client::traits::Tool {
                            name: t["name"].as_str().unwrap_or_default().to_string(),
                            description: t["description"].as_str().map(|s| s.to_string()),
                            input_schema: t["inputSchema"].clone(),
                        };
                        if let Some(ai_tool) =
                            helpers::convert_mcp_tool_to_ai_tool(server.id, &mcp_tool)
                        {
                            all_tools.push(ai_tool);
                        }
                    }
                }
                continue;
            }

            // Auto mode: Get or create MCP session and collect tools for LLM
            let session_arc = match self.session_manager
                .get_or_create_with_context(
                    *server_id,
                    context.user_id,
                    Some(context.conversation_id),
                    context.message_id,
                )
                .await
            {
                Ok(s) => s,
                Err(e) => {
                    tracing::warn!(
                        "Failed to connect to MCP server '{}': {} — skipping",
                        server.name, e
                    );
                    continue;
                }
            };
            let mut session = session_arc.write().await;

            // List tools from server
            let mcp_tools = match session.list_tools().await {
                Ok(tools) => tools,
                Err(e) => {
                    tracing::warn!(
                        "Failed to list tools from server {}: {}",
                        server.name,
                        e
                    );
                    continue; // Skip this server
                }
            };

            // Filter tools if specific tools requested
            let tools_to_add = if requested_tools.is_empty() {
                // Empty array = all tools
                mcp_tools
            } else {
                // Filter to requested tools only
                mcp_tools
                    .into_iter()
                    .filter(|t| requested_tools.contains(&t.name))
                    .collect()
            };

            // Convert and add tools (using server_id for unique tool naming).
            // `convert_mcp_tool_to_ai_tool` returns None for tools whose
            // composed `<server_id>__<tool_name>` would fail Anthropic's
            // `^[a-zA-Z0-9_-]{1,128}$` constraint — drop them from the
            // list_tools output (a silent rename would break dispatch on
            // the return path; the warning log captures the (server, tool)
            // pair).
            for mcp_tool in tools_to_add {
                if let Some(ai_tool) = helpers::convert_mcp_tool_to_ai_tool(server.id, &mcp_tool) {
                    all_tools.push(ai_tool);
                }
            }
        }

        // Append always-mode pre-fetched context to the latest USER turn (not the
        // system prefix). This context is volatile — re-fetched every request — so
        // keeping it out of the cacheable tools+system prefix preserves the prompt
        // cache (mirrors the memory-retrieval move). Falls back to a system message
        // only when there is no user turn to attach to.
        if !always_mode_context.is_empty() {
            let context_block = format!(
                "\n\n--- Pre-fetched context ---\n{}\n--- End context ---",
                always_mode_context.join("\n\n")
            );
            if let Some(user_msg) = request
                .messages
                .iter_mut()
                .rev()
                .find(|m| m.role == ai_providers::Role::User)
            {
                user_msg
                    .content
                    .push(ai_providers::ContentBlock::Text { text: context_block });
            } else {
                request.messages.push(ai_providers::ChatMessage {
                    role: ai_providers::Role::System,
                    content: vec![ai_providers::ContentBlock::Text { text: context_block }],
                });
            }
            tracing::debug!(
                "Injected {} always-mode context blocks into the user turn",
                always_mode_context.len()
            );
        }

        tracing::info!(
            "MCP extension: added {} tools from {} servers",
            all_tools.len(),
            server_configs.len()
        );

        // DEBUG: Log each tool being added
        for (i, tool) in all_tools.iter().enumerate() {
            tracing::info!(
                "Tool {}: name='{}', description='{}', schema={}",
                i,
                tool.function.name,
                tool.function.description.as_ref().unwrap_or(&"".to_string()),
                serde_json::to_string(&tool.function.parameters).unwrap_or_default()
            );
        }

        // Add tools to ChatRequest
        if !all_tools.is_empty() {
            tracing::info!("Adding {} tools to ChatRequest", all_tools.len());
            request.tools = all_tools;

            // On the first iteration, nudge the model to prefer tools over training knowledge.
            // This is a soft hint — the model can still answer directly if no tool is relevant.
            // Only injected on iteration 1 to avoid redundancy in follow-up tool-calling loops.
            if context.iteration == 1 {
                let system_addition = tool_system_guidance(&request.tools);

                if let Some(sys_msg) = request.messages.iter_mut().find(|m| m.role == ai_providers::Role::System) {
                    if let Some(ai_providers::ContentBlock::Text { text }) = sys_msg.content.first_mut() {
                        text.push_str(&system_addition);
                    }
                } else {
                    request.messages.insert(0, ai_providers::ChatMessage {
                        role: ai_providers::Role::System,
                        content: vec![ai_providers::ContentBlock::Text { text: system_addition }],
                    });
                }
            }
        } else {
            tracing::warn!("No tools to add to ChatRequest!");
        }

        Ok(BeforeLlmAction::Continue)
    }

    async fn after_llm_call(
        &self,
        context: &StreamContext,
        final_message: &Message,
        tx: Option<&tokio::sync::mpsc::UnboundedSender<Result<Event, Infallible>>>,
    ) -> Result<ExtensionAction, AppError> {
        tracing::info!(
            "MCP after_llm_call: message_id={}, conversation_id={}, iteration={}",
            final_message.id,
            context.conversation_id,
            context.iteration
        );

        // === STEP 0: Check loop settings ===
        // Get loop settings from conversation MCP settings (or use defaults)
        let loop_settings = crate::core::Repos
            .chat
            .mcp
            .get_conversation_settings(context.conversation_id)
            .await?
            .map(|s| s.get_loop_settings())
            .unwrap_or_default();

        tracing::info!(
            "Loop settings: max_iteration={}, stop_when_no_tool_calling={}, stop_when_tools_called={}",
            loop_settings.max_iteration,
            loop_settings.stop_when_no_tool_calling,
            loop_settings.stop_when_tools_called.len()
        );

        // Check max_iteration (0 = unlimited)
        if loop_settings.max_iteration > 0 && context.iteration >= loop_settings.max_iteration {
            tracing::info!(
                "Max iteration limit reached: iteration={} >= max_iteration={}",
                context.iteration,
                loop_settings.max_iteration
            );
            // finalize() already wrote tool_use blocks for the current LLM response.
            // Create synthetic error tool_results for every unexecuted tool_use so the
            // DB invariant (each TU has a matching TR) is maintained. Without this,
            // the next user message would trigger an Anthropic "tool_use without tool_result" error.
            if let Some(message_id) = context.message_id
                && let Ok(Some(msg)) = Repos.chat.core.get_message_with_content(message_id).await {
                    let executed_ids: std::collections::HashSet<String> = msg.contents.iter()
                        .filter_map(|c| c.parse_content().ok())
                        .filter_map(|cd| McpContentData::from_message_content(&cd).ok())
                        .filter_map(|mcd| match mcd {
                            McpContentData::ToolResult { tool_use_id, .. } => Some(tool_use_id),
                            _ => None,
                        })
                        .collect();
                    let pending_tool_uses: Vec<(String, String)> = msg.contents.iter()
                        .filter_map(|c| c.parse_content().ok())
                        .filter_map(|cd| McpContentData::from_message_content(&cd).ok())
                        .filter_map(|mcd| match mcd {
                            McpContentData::ToolUse { id, name, .. }
                                if !executed_ids.contains(&id) => Some((id, name)),
                            _ => None,
                        })
                        .collect();
                    for (tool_use_id, tool_name) in pending_tool_uses.iter() {
                        let error_result = McpContentData::ToolResult {
                            tool_use_id: tool_use_id.clone(),
                            name: Some(tool_name.clone()),
                            server_id: None,
                            content: "Tool execution stopped: maximum iteration limit reached."
                                .to_string(),
                            is_error: Some(true),
                                    attachment: None,
                                    images: None,
                            resource_links: None,
                            hidden_content: None,
                            structured_content: None,
                        };
                        let msg_content = error_result.to_message_content();
                        // append_content assigns sequence_order atomically (MAX+1) so these
                        // synthetic results stay strictly after the unresolved tool_use blocks.
                        if let Err(e) = Repos.chat.core.append_content(
                            message_id,
                            &msg_content.content_type(),
                            msg_content,
                        ).await {
                            tracing::error!(
                                "Failed to save synthetic tool_result for tool_use_id={}: {}",
                                tool_use_id, e
                            );
                        }
                    }
                }
            return Ok(ExtensionAction::Complete);
        }

        // === STEP 1: Check for approved pending tools (from previous approval) ===
        tracing::info!("after_llm_call: Checking for approved tools on branch {}", context.branch_id);
        let approved_pending = super::approval::repository::get_approved_tools_for_branch(
            &self.pool,
            context.branch_id,
        )
        .await?;

        tracing::info!("after_llm_call: Found {} approved tools", approved_pending.len());

        if !approved_pending.is_empty() {
            tracing::info!(
                "Found {} approved pending tools to execute in after_llm_call",
                approved_pending.len()
            );

            // Execute approved tools using shared helper
            tracing::info!("after_llm_call: Executing approved tools with tx={}", tx.is_some());
            let (tool_results, executed_ids, final_response) = self.execute_approved_tools_sync(
                &approved_pending,
                context,
                tx,
            ).await?;
            tracing::info!(
                "after_llm_call: Executed {} tools successfully, tool_use_ids: {:?}",
                tool_results.len(),
                executed_ids
            );

            // Cancel any elicitations that are still pending after tool execution ends.
            if let Some(message_id) = context.message_id {
                let _ = Repos.chat.core.cancel_pending_elicitations(message_id).await;
            }

            // If any approved tool emitted audience=["user"] content, bypass the next LLM call.
            if let Some(text) = final_response {
                return Ok(ExtensionAction::CompleteWithContent { text });
            }

            // Return Continue action to append tool results to assistant message
            tracing::info!("Returning {} approved tool results to append to assistant message", tool_results.len());
            return Ok(ExtensionAction::Continue {
                assistant_message_content: tool_results,
            });
        }

        // === STEP 2: Load message contents and find new ToolUse blocks ===
        let message_with_content = Repos
            .chat
            .core
            .get_message_with_content(final_message.id)
            .await?
            .ok_or_else(|| AppError::internal_error("Message not found after finalization"))?;

        tracing::info!(
            "Message {} has {} content blocks",
            final_message.id,
            message_with_content.contents.len()
        );

        // Did the assistant produce answer text this iteration? (Used by the
        // side-effect 3-way decision: a side-effect-only turn WITH text finalizes;
        // WITHOUT text we must loop once so the model produces an answer.) Mirror
        // collect_text's macro-safe "serialize and read type==text" pattern.
        let assistant_has_text = message_with_content.contents.iter().any(|c| {
            c.parse_content()
                .ok()
                .and_then(|d| serde_json::to_value(&d).ok())
                .map(|v| {
                    v.get("type").and_then(|t| t.as_str()) == Some("text")
                        && v.get("text")
                            .and_then(|t| t.as_str())
                            .map(|s| !s.trim().is_empty())
                            .unwrap_or(false)
                })
                .unwrap_or(false)
        });

        // Find ToolUse and ToolResult content blocks
        let mut tool_uses = Vec::new();
        let mut executed_tool_use_ids = std::collections::HashSet::new();

        // First pass: collect tool_result tool_use_ids from context metadata (executed in before_llm_call)
        if let Some(context_executed) = context.metadata.get("executed_tool_use_ids")
            && let Ok(ids) = serde_json::from_value::<Vec<String>>(context_executed.clone()) {
                tracing::info!("Found {} executed tool_use_ids in context metadata: {:?}", ids.len(), ids);
                executed_tool_use_ids.extend(ids);
            }

        // Also collect from tool_result blocks in the message (for redundancy/safety)
        for content in &message_with_content.contents {
            let content_data = content.parse_content()?;
            if let Ok(mcp_content) = McpContentData::from_message_content(&content_data)
                && let McpContentData::ToolResult { tool_use_id, .. } = mcp_content {
                    executed_tool_use_ids.insert(tool_use_id);
                }
        }

        tracing::info!(
            "Total executed tool_use_ids (from context + message): {}",
            executed_tool_use_ids.len()
        );

        // Second pass: collect tool_uses that haven't been executed yet
        for content in &message_with_content.contents {
            tracing::info!(
                "  Content block: type='{}', sequence={}",
                content.content_type,
                content.sequence_order
            );

            let content_data = content.parse_content()?;

            // Try to parse as MCP Extension content
            if let Ok(mcp_content) = McpContentData::from_message_content(&content_data) {
                tracing::info!("    Parsed as MCP content: {:?}", match &mcp_content {
                    McpContentData::ToolUse { name, .. } => format!("ToolUse({})", name),
                    McpContentData::ToolResult { name, .. } => format!("ToolResult({:?})", name),
                });

                if let McpContentData::ToolUse { id, name, server_id, input } = mcp_content {
                    // Skip tool_uses that already have a tool_result (already executed)
                    if executed_tool_use_ids.contains(&id) {
                        tracing::info!("    Skipping tool_use {} - already has result", id);
                        continue;
                    }
                    // Store server_id and name separately
                    tool_uses.push((id, name, server_id, input));
                }
            }
        }

        tracing::info!(
            "Extracted {} tool uses from message {} ({} already executed)",
            tool_uses.len(),
            final_message.id,
            executed_tool_use_ids.len()
        );

        if tool_uses.is_empty() {
            // No tool uses - check stop_when_no_tool_calling setting
            if loop_settings.stop_when_no_tool_calling {
                tracing::info!("No tool uses found and stop_when_no_tool_calling=true, conversation complete");
                return Ok(ExtensionAction::Complete);
            } else {
                tracing::info!("No tool uses found but stop_when_no_tool_calling=false, continuing anyway");
                // Continue with empty results (LLM will generate next response)
                return Ok(ExtensionAction::Continue {
                    assistant_message_content: Vec::new(),
                });
            }
        }

        // Check MCP approval settings for this conversation
        let settings = crate::core::Repos
            .chat
            .mcp
            .get_conversation_settings(context.conversation_id)
            .await?;

        // Load user defaults — used both as fallback when this conversation
        // has no per-conversation settings AND as an additional source of
        // auto-approved tools (e.g. built-in servers auto-approved at the
        // user level should be honored regardless of conversation overrides).
        let user_defaults = {
            use crate::modules::mcp::chat_extension::defaults::repository as defaults_repo;
            defaults_repo::get_user_defaults(&self.pool, context.user_id)
                .await
                .ok()
                .flatten()
        };
        let user_auto_approved: Vec<super::approval::models::AutoApprovedServer> = user_defaults
            .as_ref()
            .map(|d| d.get_auto_approved_tools())
            .unwrap_or_default();

        let (approval_mode, auto_approved_servers) = if let Some(ref settings) = settings {
            // Conversation-specific settings exist — use them verbatim.
            let servers: Vec<super::approval::models::AutoApprovedServer> =
                serde_json::from_value(settings.auto_approved_tools.clone()).unwrap_or_default();
            (settings.get_approval_mode(), servers)
        } else if let Some(ref defaults) = user_defaults {
            // No conversation override — inherit the user's defaults so the
            // approval_mode they configured in `/api/mcp/defaults` actually
            // takes effect for fresh conversations.
            (defaults.get_approval_mode(), defaults.get_auto_approved_tools())
        } else {
            // No conversation override AND no user defaults: be conservative.
            (crate::modules::mcp::chat_extension::ApprovalMode::ManualApprove, Vec::new())
        };

        tracing::info!(
            "MCP extension: {} tools, approval_mode={}, auto_approved_servers={}",
            tool_uses.len(),
            approval_mode,
            auto_approved_servers.len()
        );

        // Built-in privileged servers (files/memory/elicitation) always execute,
        // even when the conversation has MCP approval Disabled — so a user with MCP off
        // still gets file reading + memory saving.
        let has_builtin_call = tool_uses.iter().any(|(_, _, sid, _)| {
            uuid::Uuid::parse_str(sid)
                .map(is_builtin_server_id)
                .unwrap_or(false)
        });

        // Check approval mode
        if matches!(approval_mode, crate::modules::mcp::chat_extension::ApprovalMode::Disabled)
            && !has_builtin_call
        {
            tracing::info!("MCP disabled for conversation {}", context.conversation_id);
            return Ok(ExtensionAction::Complete);
        }

        // Get accessible servers for lookups (+ the auto-attached built-in
        // servers, by deterministic id, so their tool calls dispatch + execute).
        let mut accessible_servers =
            helpers::get_all_accessible_config(&self.pool, context.user_id).await?;
        for id in auto_attach_builtin_ids(&context.metadata) {
            if !accessible_servers.iter().any(|s| s.id == id) {
                if let Some(bs) = crate::core::Repos.mcp.get_any_server(id).await? {
                    // Mirror the before_llm_call guard: never resolve a disabled
                    // built-in (get_any_server ignores `enabled`). With both
                    // sites guarded a disabled built-in hits "Server not found".
                    if bs.enabled {
                        accessible_servers.push(bs);
                    }
                }
            }
        }

        // Determine which tools need approval vs can execute immediately
        let mut tools_to_execute = Vec::new();
        let mut tools_needing_approval = Vec::new();
        // Non-builtin tools called in a Disabled-approval conversation. We only
        // reach the classification loop in Disabled mode when a built-in call
        // shared the turn (the early return above handles the builtin-free case),
        // so a third-party tool here must NOT run AND must NOT surface an approval
        // prompt (the user turned MCP off) — it gets a synthesized denial
        // tool_result instead, keeping the Disabled contract honest while still
        // pairing every tool_use with a tool_result.
        let mut tools_disabled = Vec::new();

        for (tool_use_id, tool_name, server_id, input) in tool_uses {
            // Privileged built-in servers bypass approval entirely.
            let is_builtin = uuid::Uuid::parse_str(&server_id)
                .map(is_builtin_server_id)
                .unwrap_or(false);

            // Disabled mode + non-builtin → deny (no run, no prompt).
            if !is_builtin
                && matches!(
                    approval_mode,
                    crate::modules::mcp::chat_extension::ApprovalMode::Disabled
                )
            {
                tools_disabled.push((tool_use_id, tool_name, server_id));
                continue;
            }

            let needs_approval = if is_builtin {
                false
            } else {
                match approval_mode {
                    crate::modules::mcp::chat_extension::ApprovalMode::AutoApprove => false,
                    crate::modules::mcp::chat_extension::ApprovalMode::ManualApprove => {
                        // Check if this tool is auto-approved using server_id directly
                        let is_auto_approved = if let Ok(sid) = uuid::Uuid::parse_str(&server_id) {
                            auto_approved_servers
                                .iter()
                                .any(|s| s.server_id == sid && s.contains_tool(&tool_name))
                                || user_auto_approved
                                    .iter()
                                    .any(|s| s.server_id == sid && s.contains_tool(&tool_name))
                        } else {
                            false
                        };
                        tracing::info!(
                            "Tool '{}' (server={}) auto-approved check: is_auto_approved={}",
                            tool_name,
                            server_id,
                            is_auto_approved
                        );
                        !is_auto_approved
                    }
                    // Handled by the Disabled-deny branch above.
                    crate::modules::mcp::chat_extension::ApprovalMode::Disabled => {
                        unreachable!("Disabled non-builtin tools are denied above")
                    }
                }
            };

            tracing::info!(
                "Tool '{}' (server={}, id={}): needs_approval={}",
                tool_name,
                server_id,
                tool_use_id,
                needs_approval
            );

            if needs_approval {
                tools_needing_approval.push((tool_use_id, tool_name.clone(), server_id.clone(), input));
            } else {
                tools_to_execute.push((tool_use_id, tool_name, server_id, input));
            }
        }

        // Create pending approval records for tools that need manual approval
        if !tools_needing_approval.is_empty() {
            tracing::info!(
                "Creating {} pending approval records",
                tools_needing_approval.len()
            );

            for (tool_use_id, tool_name, server_id_str, input) in &tools_needing_approval {
                // Parse UUID and lookup server name
                let (server_id, server_name) = if let Ok(id) = uuid::Uuid::parse_str(server_id_str) {
                    let name = accessible_servers
                        .iter()
                        .find(|s| s.id == id)
                        .map(|s| s.name.clone())
                        .unwrap_or_else(|| id.to_string());
                    (Some(id), name)
                } else {
                    (None, server_id_str.to_string())
                };

                // Create pending approval with server_id and server_name
                tracing::info!(
                    "Creating approval record: tool_use_id={}, branch_id={}, message_id={}, tool_name={}",
                    tool_use_id, context.branch_id, final_message.id, tool_name
                );

                let approval_record = crate::core::Repos
                    .chat
                    .mcp
                    .create_tool_approval(
                        context.conversation_id,
                        context.branch_id,
                        final_message.id,
                        context.user_id,
                        tool_use_id.clone(),
                        tool_name.clone(),
                        input.clone(),
                        server_id,
                        server_name.clone(),
                    )
                    .await?;

                tracing::info!(
                    "Created approval record: id={}, tool_use_id={}, branch_id={}, status={}",
                    approval_record.id, approval_record.tool_use_id, approval_record.branch_id, approval_record.status
                );

                // Send SSE event for approval required
                helpers::send_approval_required_event(tx, tool_use_id, tool_name, &server_name, server_id_str, input).await?;
            }

            // Do NOT pause here. A built-in tool (files/memory) can share the
            // turn with a third-party tool awaiting approval; its `tool_use` was
            // already finalized to the DB and bypasses approval by design. We
            // must execute it + persist its `tool_result` FIRST (the execution
            // loop below) so the next provider request doesn't fail with
            // "tool_use ids found without tool_result blocks". The pause happens
            // AFTER the loop (search: "Pause for pending approvals").
            tracing::info!(
                "{} tool(s) await approval; executing approval-exempt tools first, then pausing",
                tools_needing_approval.len()
            );
        }

        tracing::info!("MCP extension: executing {} auto-approved tools", tools_to_execute.len());

        // accessible_servers already available from above

        // Execute each auto-approved tool and collect results
        let mut tool_results = Vec::new();

        // Disabled-mode non-builtin tools (mixed builtin/third-party turn): emit a
        // denial tool_result so the tool_use isn't orphaned, without running the
        // tool or prompting for approval. The built-in(s) in `tools_to_execute`
        // still execute below.
        for (tool_use_id, tool_name, server_id_str) in &tools_disabled {
            let denial = McpContentData::ToolResult {
                tool_use_id: tool_use_id.clone(),
                name: Some(tool_name.clone()),
                server_id: Some(server_id_str.clone()),
                content: "MCP is disabled for this conversation; tool not executed."
                    .to_string(),
                is_error: Some(true),
                attachment: None,
                images: None,
                resource_links: None,
                hidden_content: None,
                structured_content: None,
            };
            tool_results.push(denial.to_message_content());
        }

        let mut final_response_text: Option<String> = None;
        // Track every tool executed this iteration so we can detect the
        // "only side-effect tools were called" case (Track B inline self-save):
        // `remember`/`forget` don't need the model to see their result, so when
        // ONLY those ran we finalize without a no-op continuation round-trip.
        // (server_id, tool_name) of every tool actually dispatched this turn —
        // the server id is needed to scope the side-effect check to the memory
        // built-in (a third-party `remember` must not finalize the loop).
        let mut executed_tools: Vec<(Uuid, String)> = Vec::new();

        // Channel for elicitation DB persistence (http.rs → mcp.rs via Repos)
        let (elicit_notify_tx, mut elicit_notify_rx) =
            tokio::sync::mpsc::unbounded_channel::<ElicitationStartedNotification>();
        let bind_user_id = context.user_id;
        tokio::spawn(async move {
            while let Some(notif) = elicit_notify_rx.recv().await {
                // Bind the calling user_id to the elicitation entry so
                // the /respond handler can verify the responder is the
                // user who initiated the chat call. Closes
                // 02-permissions F-04.
                crate::modules::mcp::elicitation::registry::bind_owner(
                    notif.elicitation_id,
                    bind_user_id,
                );
                if let Some(msg_id) = notif.message_id {
                    let content_data = MessageContentData::ElicitationRequest {
                        elicitation_id: notif.elicitation_id.to_string(),
                        message: notif.message,
                        requested_schema: notif.requested_schema,
                        server: notif.server,
                        status: "pending".to_string(),
                        response_content: None,
                    };
                    let _ = crate::core::Repos.chat.core
                        .append_content_with_id(notif.content_id, msg_id, "elicitation_request", content_data)
                        .await;
                }
            }
        });

        for (tool_use_id, tool_name, server_id_str, input) in tools_to_execute {
            // Parse UUID
            let server_id = match uuid::Uuid::parse_str(&server_id_str) {
                Ok(id) => id,
                Err(_) => {
                    tracing::error!("Invalid server_id: {}", server_id_str);
                    continue;
                }
            };
            executed_tools.push((server_id, tool_name.clone()));

            // Find server by ID
            let server = accessible_servers
                .iter()
                .find(|s| s.id == server_id);

            if server.is_none() {
                tracing::error!("Server not found for tool: {}", tool_name);
                // Create error result
                let error_result = McpContentData::ToolResult {
                    tool_use_id: tool_use_id.clone(),
                    name: Some(tool_name.clone()),
                    server_id: Some(server_id_str.clone()),
                    content: format!("Server '{}' not found", server_id),
                    is_error: Some(true),
                    attachment: None,
                    images: None,
                    resource_links: None,
                    hidden_content: None,
                    structured_content: None,
                };
                tool_results.push(error_result.to_message_content());
                continue;
            }

            let server = server.unwrap();

            // Send tool start event
            helpers::send_tool_start_event(tx, &tool_use_id, &tool_name, &server.name, &input).await;

            let (mut result, is_final) = if server.id
                == crate::modules::elicitation_mcp::elicitation_mcp_server_id()
                && tool_name == "ask_user"
            {
                // `ask_user` is driven INLINE (it needs the live chat sse_tx) and is
                // never dispatched over the loopback — so intercept here, BEFORE any
                // session is created, to skip a wasted initialize handshake. (The
                // same interception lives defensively in execute_tool for the
                // sampling + before_llm_call approved-tools paths.)
                (
                    helpers::run_ask_user_elicitation(
                        input,
                        context.message_id,
                        Some(context.user_id),
                        tx.cloned(),
                        Some(elicit_notify_tx.clone()),
                    )
                    .await,
                    false,
                )
            } else if server.supports_sampling {
                // Sampling path: create a fresh session with the sampling handler (bypass pool)
                let model_id_opt = context.metadata.get("model_id")
                    .and_then(|v| v.as_str())
                    .and_then(|s| uuid::Uuid::parse_str(s).ok());

                if let Some(model_id) = model_id_opt {
                    // Acquire session guard (enforces max_concurrent_sessions if set)
                    match acquire_session(server.id, server.max_concurrent_sessions) {
                        Err(e) => {
                            tracing::warn!("Sampling session limit reached for server {}: {}", server.name, e);
                            (McpContentData::ToolResult {
                                tool_use_id: tool_use_id.clone(),
                                name: Some(tool_name.clone()),
                                server_id: Some(server.id.to_string()),
                                content: e.to_string(),
                                is_error: Some(true),
                                            attachment: None,
                                            images: None,
                                resource_links: None,
                                hidden_content: None,
                                structured_content: None,
                            }, false)
                        }
                        Ok(_guard) => {
                            // _guard keeps the session counter incremented until end of block
                            match ChatSamplingHandler::new(model_id, context.user_id).await {
                                Err(e) => {
                                    tracing::warn!("[sampling] Failed to init provider for '{}': {}", server.name, e);
                                    (McpContentData::ToolResult {
                                        tool_use_id: tool_use_id.clone(),
                                        name: Some(tool_name.clone()),
                                        server_id: Some(server.id.to_string()),
                                        content: format!("Failed to initialize sampling provider: {}", e),
                                        is_error: Some(true),
                                                            attachment: None,
                                                            images: None,
                                        resource_links: None,
                                        hidden_content: None,
                                        structured_content: None,
                                    }, false)
                                }
                                Ok(h) => {
                                    match McpSession::new_with_sampling(server.clone(), Arc::new(h)).await {
                                        Ok(mut sampling_session) => {
                                            helpers::execute_tool(
                                                &mut sampling_session,
                                                &tool_name,
                                                input,
                                                &server.name,
                                                Some(server.timeout_seconds),
                                                context.message_id,
                                                tx.cloned(),
                                                Some(elicit_notify_tx.clone()),
                                            )
                                            .await
                                        }
                                        Err(e) => {
                                            tracing::error!("Failed to create sampling session for {}: {}", server.name, e);
                                            (McpContentData::ToolResult {
                                                tool_use_id: tool_use_id.clone(),
                                                name: Some(tool_name.clone()),
                                                server_id: Some(server.id.to_string()),
                                                content: format!("Failed to connect to server: {}", e),
                                                is_error: Some(true),
                                                                            attachment: None,
                                                                            images: None,
                                                resource_links: None,
                                                hidden_content: None,
                                                structured_content: None,
                                            }, false)
                                        }
                                    }
                                }
                            }
                        }
                    }
                } else {
                    tracing::warn!(
                        "[sampling] Server '{}' has supports_sampling=true but no model_id in context; cannot execute sampling tool",
                        server.name
                    );
                    (McpContentData::ToolResult {
                        tool_use_id: tool_use_id.clone(),
                        name: Some(tool_name.clone()),
                        server_id: Some(server.id.to_string()),
                        content: "Cannot execute sampling tool: no model available in context. Ensure a model is selected.".to_string(),
                        is_error: Some(true),
                            attachment: None,
                            images: None,
                        resource_links: None,
                        hidden_content: None,
                        structured_content: None,
                    }, false)
                }
            } else {
                // Non-sampling path: use session manager (creates ephemeral session with context
                // headers for built-in servers; ephemeral non-pooled session for external servers)
                match self.session_manager
                    .get_or_create_with_context(
                        server.id,
                        context.user_id,
                        Some(context.conversation_id),
                        context.message_id,
                    )
                    .await
                {
                    Err(e) => {
                        tracing::warn!(
                            "Failed to get session for MCP server '{}': {}",
                            server.name, e
                        );
                        (McpContentData::ToolResult {
                            tool_use_id: tool_use_id.clone(),
                            name: Some(tool_name.clone()),
                            server_id: Some(server.id.to_string()),
                            content: format!("Failed to connect to server: {}", e),
                            is_error: Some(true),
                                    attachment: None,
                                    images: None,
                            resource_links: None,
                            hidden_content: None,
                            structured_content: None,
                        }, false)
                    }
                    Ok(session_arc) => {
                        let mut session = session_arc.write().await;
                        helpers::execute_tool(&mut session, &tool_name, input, &server.name, Some(server.timeout_seconds), context.message_id, tx.cloned(), Some(elicit_notify_tx.clone())).await
                    }
                }
            };

            // Set tool_use_id and server_id
            if let McpContentData::ToolResult {
                tool_use_id: ref mut id,
                server_id: ref mut sid,
                is_error,
                ref content,
                ..
            } = result
            {
                *id = tool_use_id.clone();
                *sid = Some(server.id.to_string());

                // Send tool complete event
                helpers::send_tool_complete_event(
                    tx,
                    &tool_use_id,
                    &tool_name,
                    &server.name,
                    is_error.unwrap_or(false),
                    Some(content.as_str()),
                )
                .await;
            }

            // Generic resource_link handling: fetch-and-save any resource_links returned by a tool.
            // Works uniformly for built-in servers (short-lived JWT auth) and external MCP servers
            // (server-configured headers). Runs the full processing pipeline (text extraction,
            // thumbnails) and creates a permanent DB artifact visible to the user.
            // Exception: is_saved=true links already exist in originals storage — skip all processing.
            let mut saved_artifacts: Vec<(Uuid, String, Option<String>)> = Vec::new(); // (artifact_id, display_name, download_url)
            let mut saved_file_urls: Vec<(String, String)> = Vec::new(); // (display_name, download_url) for is_saved links
            // (link_index, artifact_id) for workspace artifacts saved by this
            // pipeline. Applied back onto resource_links[i].file_id after the loop
            // so the browser inline preview can fetch via the authenticated,
            // same-origin /api/files/{id}/... path (the tool-emitted absolute
            // loopback URI is unreachable from the browser).
            // (link_idx, file_id, version, version_id) — version/version_id pin
            // the inline preview's resource_link to the exact artifact version.
            let mut artifact_file_ids: Vec<(usize, Uuid, i32, Uuid)> = Vec::new();
            if let McpContentData::ToolResult { ref resource_links, is_error, .. } = result
                && !is_error.unwrap_or(false)
                    && let Some(links) = resource_links {
                        for (link_idx, link) in links.iter().enumerate() {

                        // is_saved=true: file already exists in originals storage.
                        // URI is a download-with-token URL — skip fetch/process/save pipeline.
                        if link.is_saved == Some(true) {
                            let name = link.name.as_deref().unwrap_or("file").to_string();
                            saved_file_urls.push((name, link.uri.clone()));
                            continue;
                        }

                        use crate::modules::file::models::FileCreateData;
                        use crate::modules::file::processing::ProcessingManager;
                        use crate::modules::file::storage::manager::get_file_storage;

                        // Build auth headers appropriate for the server type
                        let mut fetch_headers = reqwest::header::HeaderMap::new();
                        if server.is_built_in {
                            match McpSessionManager::generate_short_lived_jwt(
                                context.user_id, &self.config.jwt.secret, 10
                            ) {
                                Ok(token) => {
                                    if let Ok(hval) = reqwest::header::HeaderValue::from_str(
                                        &format!("Bearer {}", token)
                                    ) {
                                        fetch_headers.insert(reqwest::header::AUTHORIZATION, hval);
                                    }
                                    if let Ok(hval) = reqwest::header::HeaderValue::from_str(
                                        &context.conversation_id.to_string()
                                    ) {
                                        fetch_headers.insert(
                                            reqwest::header::HeaderName::from_static("x-conversation-id"),
                                            hval,
                                        );
                                    }
                                }
                                Err(e) => {
                                    tracing::warn!("Failed to generate JWT for resource_link fetch: {}", e);
                                }
                            }
                        } else if let Some(headers_map) = server.headers.as_object() {
                            for (key, value) in headers_map.iter() {
                                if let Some(val_str) = value.as_str()
                                    && let (Ok(hname), Ok(hval)) = (
                                        reqwest::header::HeaderName::from_bytes(key.as_bytes()),
                                        reqwest::header::HeaderValue::from_str(val_str),
                                    ) {
                                        fetch_headers.insert(hname, hval);
                                    }
                            }
                        }

                        match reqwest::Client::builder()
                            .default_headers(fetch_headers)
                            .build()
                        {
                            Ok(client) => {
                                match client.get(&link.uri).send().await {
                                    Ok(response) if response.status().is_success() => {
                                        let content_type_mime = response
                                            .headers()
                                            .get(reqwest::header::CONTENT_TYPE)
                                            .and_then(|v| v.to_str().ok())
                                            .and_then(|s| s.split(';').next())
                                            .map(|s| s.trim().to_string());

                                        match response.bytes().await {
                                            Ok(bytes) => {
                                                let bytes = bytes.to_vec();
                                                let display_name =
                                                    link.name.as_deref().unwrap_or("file");
                                                // Canonical extension (rsplit + lowercase) — MUST match how
                                                // the download/read paths derive the blob key. Path::extension
                                                // would save dotfiles / no-extension names (`.bashrc`,
                                                // `Makefile`) as `…​.bin` but load them as `…​.bashrc` → 404.
                                                let ext =
                                                    crate::modules::file::utils::extension_of(display_name);
                                                let mime_type = content_type_mime.or_else(|| {
                                                    mime_guess::from_ext(&ext)
                                                        .first()
                                                        .map(|m| m.to_string())
                                                });
                                                let mime_type_str = mime_type
                                                    .as_deref()
                                                    .unwrap_or("application/octet-stream");

                                                let processing_result = ProcessingManager::new()
                                                    .process_file(&bytes, mime_type_str)
                                                    .await
                                                    .unwrap_or_default();

                                                let artifact_id = Uuid::new_v4();
                                                let storage = get_file_storage();

                                                match storage
                                                    .save_original(
                                                        context.user_id,
                                                        artifact_id,
                                                        &ext,
                                                        &bytes,
                                                    )
                                                    .await
                                                {
                                                    Ok(_) => {
                                                        for (n, text) in processing_result
                                                            .text_pages
                                                            .iter()
                                                            .enumerate()
                                                        {
                                                            let _ = storage
                                                                .save_text_page(
                                                                    context.user_id,
                                                                    artifact_id,
                                                                    (n + 1) as u32,
                                                                    text,
                                                                )
                                                                .await;
                                                        }
                                                        if let Some(thumb) = processing_result
                                                            .thumbnails
                                                            .first()
                                                        {
                                                            let _ = storage
                                                                .save_image(
                                                                    context.user_id,
                                                                    artifact_id,
                                                                    1,
                                                                    true,
                                                                    thumb,
                                                                )
                                                                .await;
                                                        }
                                                        for (n, img) in processing_result
                                                            .images
                                                            .iter()
                                                            .enumerate()
                                                        {
                                                            let _ = storage
                                                                .save_image(
                                                                    context.user_id,
                                                                    artifact_id,
                                                                    (n + 1) as u32,
                                                                    false,
                                                                    img,
                                                                )
                                                                .await;
                                                        }

                                                        let file_size = bytes.len() as i64;
                                                        // Real checksum: version-back's no-op check compares the
                                                        // workspace bytes' checksum to the base version's. A `None`
                                                        // base never matches → every staged artifact would spuriously
                                                        // version-back even when unchanged.
                                                        let checksum =
                                                            storage.calculate_checksum(&bytes);
                                                        match Repos
                                                            .file
                                                            .create(FileCreateData {
                                                                id: artifact_id,
                                                                user_id: context.user_id,
                                                                filename: display_name
                                                                    .to_string(),
                                                                file_size,
                                                                mime_type: mime_type.clone(),
                                                                checksum: Some(checksum),
                                                                has_thumbnail:
                                                                    !processing_result
                                                                        .thumbnails
                                                                        .is_empty(),
                                                                preview_page_count:
                                                                    processing_result
                                                                        .images
                                                                        .len()
                                                                        as i32,
                                                                text_page_count:
                                                                    processing_result
                                                                        .text_pages
                                                                        .len()
                                                                        as i32,
                                                                processing_metadata:
                                                                    serde_json::to_value(
                                                                        &processing_result
                                                                            .metadata,
                                                                    )
                                                                    .unwrap_or_default(),
                                                                source_message_id:
                                                                    context.message_id,
                                                                created_by: "mcp".to_string(),
                                                            })
                                                            .await
                                                        {
                                                            Ok(file) => {
                                                                helpers::send_artifact_created_event(
                                                                    tx,
                                                                    &tool_use_id,
                                                                    &artifact_id.to_string(),
                                                                    display_name,
                                                                    mime_type.as_deref(),
                                                                    file_size,
                                                                )
                                                                .await;

                                                                // Notify the user's OTHER devices a new file exists
                                                                // (cross-device sync), mirroring files_mcp's create
                                                                // path — send_artifact_created_event above only reaches
                                                                // THIS conversation's SSE stream.
                                                                crate::modules::file::sync::publish_file_changed(
                                                                    context.user_id,
                                                                    artifact_id,
                                                                );

                                                                // No FileAttachment block is emitted for artifacts: the
                                                                // inline preview (resource_link, stamped after the loop)
                                                                // is the single UI view. Record index→(file_id, version,
                                                                // version_id) so it can pin the exact version created here.
                                                                artifact_file_ids.push((
                                                                    link_idx,
                                                                    artifact_id,
                                                                    file.version,
                                                                    file.current_version_id,
                                                                ));

                                                                tracing::info!(
                                                                    "Artifact saved from resource_link: file_id={}, filename={}",
                                                                    artifact_id, display_name
                                                                );
                                                                let download_url = {
                                                                    use jsonwebtoken::{encode, EncodingKey, Header as JwtHeader};
                                                                    use crate::modules::file::types::{DownloadTokenClaims, DOWNLOAD_TOKEN_AUDIENCE};
                                                                    let now = chrono::Utc::now().timestamp() as usize;
                                                                    let claims = DownloadTokenClaims {
                                                                        file_id: artifact_id.to_string(),
                                                                        user_id: context.user_id.to_string(),
                                                                        version: None,
                                                                        exp: now + 3600,
                                                                        iat: now,
                                                                        iss: self.config.jwt.issuer.clone(),
                                                                        aud: DOWNLOAD_TOKEN_AUDIENCE.to_string(),
                                                                    };
                                                                    // Root the tool-to-tool download URL at the SAME origin
                                                                    // get_resource_link uses (public_base_url when set, else the
                                                                    // pinned 127.0.0.1 loopback) — NOT self.config.server.host,
                                                                    // which may be 0.0.0.0 / a bind address unreachable by the
                                                                    // (possibly remote) MCP server the LLM passes this URL to.
                                                                    let origin = file_download_origin(
                                                                        self.config.code_sandbox.as_ref(),
                                                                        self.config.server.port,
                                                                    );
                                                                    encode(
                                                                        &JwtHeader::default(),
                                                                        &claims,
                                                                        &EncodingKey::from_secret(self.config.jwt.secret.as_bytes()),
                                                                    )
                                                                    .ok()
                                                                    .map(|token| build_artifact_download_url(
                                                                        &origin,
                                                                        &self.config.server.api_prefix,
                                                                        artifact_id,
                                                                        &token,
                                                                    ))
                                                                };
                                                                saved_artifacts.push((artifact_id, display_name.to_string(), download_url));
                                                            }
                                                            Err(e) => {
                                                                tracing::error!(
                                                                    "Failed to create file DB record for resource_link: {}",
                                                                    e
                                                                );
                                                            }
                                                        }
                                                    }
                                                    Err(e) => {
                                                        tracing::error!(
                                                            "Failed to save artifact original: {}",
                                                            e
                                                        );
                                                    }
                                                }
                                            }
                                            Err(e) => {
                                                tracing::warn!(
                                                    "Failed to read resource_link response body: {}",
                                                    e
                                                );
                                            }
                                        }
                                    }
                                    Ok(response) => {
                                        tracing::error!(
                                            "resource_link fetch returned HTTP {} for '{}': artifact NOT saved",
                                            response.status(),
                                            link.uri
                                        );
                                    }
                                    Err(e) => {
                                        tracing::error!(
                                            "Failed to fetch resource_link '{}': {} — artifact NOT saved",
                                            link.uri, e
                                        );
                                    }
                                }
                            }
                            Err(e) => {
                                tracing::error!(
                                    "Failed to build HTTP client for resource_link fetch: {}",
                                    e
                                );
                            }
                        }
                        } // end for link in links
                    }

            // Update tool result content with the saved artifact info so the LLM knows the file_ids.
            // Also set hidden_content with token-based download URLs — included in LLM messages
            // but stripped from browser API responses.
            // saved_file_urls holds download-with-token URLs for is_saved=true links (no pipeline needed).
            if (!saved_artifacts.is_empty() || !saved_file_urls.is_empty())
                && let McpContentData::ToolResult { ref mut content, ref mut hidden_content, ref mut resource_links, .. } = result {
                    // Stamp each saved artifact's file_id onto its resource_link so
                    // the UI inline preview fetches the content via the authenticated
                    // /api/files/{id}/... path instead of the unreachable absolute
                    // loopback URI emitted by the tool.
                    if let Some(links) = resource_links {
                        for (idx, fid, ver, ver_id) in &artifact_file_ids {
                            if let Some(l) = links.get_mut(*idx) {
                                l.file_id = Some(*fid);
                                l.version = Some(*ver);
                                l.version_id = Some(*ver_id);
                            }
                        }
                    }
                    if !saved_artifacts.is_empty() {
                        let file_descriptions: Vec<String> = saved_artifacts
                            .iter()
                            .map(|(id, name, _)| format!("'{}' (file_id: {})", name, id))
                            .collect();
                        *content = format!(
                            "Files from MCP tool have been saved as artifact attachments: {}. \
                             They will be shown as inline file previews in the UI — do not embed them inline in your response.",
                            file_descriptions.join(", ")
                        );
                    }
                    let mut url_lines: Vec<String> = saved_artifacts
                        .iter()
                        .filter_map(|(_, name, url)| url.as_ref().map(|u| format!("{} - {}", name, u)))
                        .collect();
                    for (name, url) in &saved_file_urls {
                        url_lines.push(format!("{} - {}", name, url));
                    }
                    if !url_lines.is_empty() {
                        *hidden_content = Some(format!(
                            "[system: Files saved as artifact attachments (shown as file cards in UI). \
                             Do NOT embed file URLs or images inline in your text response. \
                             To pass one of these files to another tool, copy its URL below \
                             VERBATIM into that tool's file/URL argument — never rewrite the host, \
                             never substitute 127.0.0.1/localhost, and never invent a DRS or \
                             platform URL. The URLs below are already reachable exactly as given \
                             (do not call get_resource_link for these — use the URL here directly):\n{}]",
                            url_lines.join("\n")
                        ));
                    }
                }

            // Capture user-only-audience text before converting to MessageContentData
            if is_final
                && let McpContentData::ToolResult { ref content, .. } = result {
                    tracing::info!(
                        "audience=[\"user\"]: tool '{}' on server '{}' marked as final, will bypass LLM",
                        tool_name, server.name
                    );
                    final_response_text = Some(content.clone());
                }

            // Convert to MessageContentData and add to results
            tool_results.push(result.to_message_content());

            // Check stop_when_tools_called
            if loop_settings.stop_when_tools_called.iter().any(|stop_tool| {
                stop_tool.server_id == server_id && stop_tool.tool_name == tool_name
            }) {
                tracing::info!(
                    "Tool '{}' on server '{}' matches stop_when_tools_called, will complete after this iteration",
                    tool_name,
                    server_id
                );
                // Save accumulated tool_results to DB so tool_use blocks are not orphaned.
                // finalize() already wrote tool_use blocks; without matching tool_result blocks
                // the next LLM request will be rejected by Anthropic with "tool_use without tool_result".
                // append_content assigns sequence_order atomically (MAX+1) so results stay
                // strictly after the tool_use blocks finalize() just wrote.
                if let Some(message_id) = context.message_id {
                    for tr in tool_results.iter() {
                        let _ = Repos.chat.core.append_content(
                            message_id,
                            &tr.content_type(),
                            tr.clone(),
                        ).await;
                    }
                }
                return Ok(ExtensionAction::Complete);
            }
        }

        // Pause for pending approvals (AFTER the execution loop). Built-in
        // approval-exempt tools have now executed and their results sit in
        // `tool_results`. Persist them so the built-in `tool_use` blocks are not
        // orphaned, then pause for the third-party tools awaiting approval. When
        // the user approves, the resume path executes those; the built-in result
        // is already on disk so the next request is protocol-valid.
        if !tools_needing_approval.is_empty() {
            if let Some(message_id) = context.message_id {
                for tr in tool_results.iter() {
                    let _ = Repos
                        .chat
                        .core
                        .append_content(message_id, &tr.content_type(), tr.clone())
                        .await;
                }
            }
            tracing::info!(
                "Conversation paused after executing {} approval-exempt tool result(s); waiting for {} approval(s)",
                tool_results.len(),
                tools_needing_approval.len()
            );
            return Ok(ExtensionAction::Complete);
        }

        // If any tool emitted audience=["user"] content, process references and bypass the LLM.
        // We must persist tool_results to DB BEFORE returning CompleteWithContent so that the
        // tool_use already stored by finalize() has a matching tool_result. Without this, the
        // next message's history reconstruction would see an unmatched tool_use and the API would
        // reject the request with "tool_use ids found without tool_result blocks".
        if let Some(text) = final_response_text {
            if let Some(message_id) = context.message_id {
                for result in tool_results.iter() {
                    let content_type = result.content_type();
                    if let Err(e) = Repos.chat.core.append_content(
                        message_id,
                        &content_type,
                        result.clone(),
                    ).await {
                        tracing::error!("Failed to save tool result before CompleteWithContent: {}", e);
                    }
                }
                let _ = Repos.chat.core.cancel_pending_elicitations(message_id).await;
            }
            return Ok(ExtensionAction::CompleteWithContent { text });
        }

        // Side-effect-only iteration (Track B inline self-save): if EVERY tool
        // executed this turn was a side-effect tool (remember/forget), persist
        // their tool_results (so the tool_use blocks aren't orphaned) and
        // finalize WITHOUT a continuation round-trip — the model already produced
        // its answer this iteration. A mixed call (e.g. remember + read_file) is
        // NOT side-effect-only, so it falls through to Continue and the read_file
        // result reaches the model as normal.
        if !executed_tools.is_empty()
            && executed_tools
                .iter()
                .all(|(sid, n)| is_side_effect_tool(*sid, n))
        {
            if assistant_has_text {
                // Side-effect tools + the model already gave its answer this turn:
                // persist the canned results and finalize without re-invoking.
                if let Some(message_id) = context.message_id {
                    for tr in tool_results.iter() {
                        let _ = Repos
                            .chat
                            .core
                            .append_content(message_id, &tr.content_type(), tr.clone())
                            .await;
                    }
                    let _ = Repos.chat.core.cancel_pending_elicitations(message_id).await;
                }
                return Ok(ExtensionAction::Complete);
            }
            // Side-effect-only but NO answer text → fall through to Continue so the
            // loop runs once more and the model produces an answer (the one case
            // that must continue). The tool_results ride along in that Continue.
        }

        // Cancel any elicitations that are still pending after all tools have been executed.
        if let Some(message_id) = context.message_id {
            let _ = Repos.chat.core.cancel_pending_elicitations(message_id).await;
        }

        // Return Continue action to append tool results to assistant message
        Ok(ExtensionAction::Continue {
            assistant_message_content: tool_results,
        })
    }

    fn convert_extension_content(&self, content: &Value) -> Option<ContentBlock> {
        // Check if this is MCP content by type field
        let content_type = content.get("type")?.as_str()?;
        if !matches!(content_type, "tool_use" | "tool_result") {
            return None;
        }

        // Deserialize to McpContentData and convert to ContentBlock
        serde_json::from_value::<McpContentData>(content.clone())
            .ok()
            .and_then(|mcp_content| mcp_content.to_content_block())
    }

    fn convert_from_content_block(&self, block: &ContentBlock) -> Option<MessageContentData> {
        // Try to convert ContentBlock to McpContentData
        McpContentData::from_content_block(block)
            .map(|mcp_content| mcp_content.to_message_content())
    }

    async fn process_delta(
        &self,
        delta: &ai_providers::ContentBlockDelta,
        _context: &StreamContext,
    ) -> Result<Option<ContentBlockDelta>, AppError> {
        // Convert ai-providers ToolUseDelta to our ContentBlockDelta::ToolUseDelta
        match delta {
            ai_providers::ContentBlockDelta::ToolUseDelta {
                index,
                id,
                name,
                input_delta,
            } => {
                tracing::info!(
                    "MCP process_delta: Converting ToolUseDelta at index {}, id={:?}, name={:?}",
                    index,
                    id,
                    name
                );
                Ok(Some(ContentBlockDelta::ToolUseDelta {
                    index: *index,
                    id: id.clone(),
                    name: name.clone(),
                    input_delta: input_delta.clone(),
                }))
            }
            _ => Ok(None), // Not a tool use delta
        }
    }

    async fn accumulate_delta(
        &self,
        delta: &ContentBlockDelta,
        context: &StreamContext,
    ) -> Result<(), AppError> {
        tracing::info!(
            "MCP accumulate_delta called with delta variant: {}",
            match delta {
                ContentBlockDelta::ToolUseDelta { .. } => "ToolUseDelta",
                _ => "Other",
            }
        );

        // Only accumulate ToolUseDelta variants
        if let ContentBlockDelta::ToolUseDelta {
            index,
            id,
            name,
            input_delta,
        } = delta
        {
            // Get message_id from context
            let message_id = context
                .message_id
                .ok_or_else(|| AppError::internal_error("No message_id in context"))?;

            tracing::info!(
                "MCP accumulate_delta: Accumulating ToolUseDelta for message_id={}, index={}, id={:?}, name={:?}",
                message_id,
                index,
                id,
                name
            );

            let key = (message_id, *index);

            // Lock accumulator and update
            let mut accumulator = self
                .tool_use_accumulator
                .lock()
                .map_err(|e| AppError::internal_error(format!("Failed to lock accumulator: {}", e)))?;

            let entry = accumulator.entry(key).or_insert_with(Default::default);

            // Accumulate fields
            if let Some(id) = id {
                entry.id = Some(id.clone());
            }
            if let Some(name) = name {
                entry.name = Some(name.clone());
            }
            if let Some(input_delta) = input_delta {
                entry.input_json.push_str(input_delta);
            }

            tracing::debug!(
                "MCP: Accumulated tool use delta at index {}: id={:?}, name={:?}, input_len={}",
                index,
                entry.id,
                entry.name,
                entry.input_json.len()
            );
        }

        Ok(())
    }

    async fn get_accumulated_content(
        &self,
        context: &StreamContext,
    ) -> Result<Vec<(usize, MessageContentData)>, AppError> {
        // Get message_id from context
        let message_id = context
            .message_id
            .ok_or_else(|| AppError::internal_error("No message_id in context"))?;

        // Lock accumulator and extract all entries for this message
        let mut accumulator = self
            .tool_use_accumulator
            .lock()
            .map_err(|e| AppError::internal_error(format!("Failed to lock accumulator: {}", e)))?;

        let mut content_blocks = Vec::new();

        // Collect keys to remove (entries for this message)
        let keys_to_remove: Vec<_> = accumulator
            .keys()
            .filter(|(msg_id, _)| *msg_id == message_id)
            .copied()
            .collect();

        // Extract and convert each accumulated tool use
        for key in keys_to_remove {
            let (_, index) = key;
            if let Some(accumulated) = accumulator.remove(&key) {
                // Parse accumulated JSON input
                let input = serde_json::from_str(&accumulated.input_json).unwrap_or_else(|e| {
                    tracing::error!(
                        "Failed to parse accumulated tool use input JSON: {}. Input: {}",
                        e,
                        accumulated.input_json
                    );
                    serde_json::json!({}) // Fallback to empty object
                });

                // Parse server_id and tool name from accumulated name (format: server_id__tool_name)
                let full_name = accumulated.name.unwrap_or_default();
                let mut parts = full_name.splitn(2, "__");
                let (server_id, tool_name) = match (parts.next(), parts.next()) {
                    (Some(id), Some(name)) => (id.to_string(), name.to_string()),
                    _ => {
                        tracing::warn!("[mcp] Tool name missing server_id prefix: {}", full_name);
                        (String::new(), full_name)
                    }
                };

                // Create McpContentData::ToolUse with separate server_id and name
                let tool_use = McpContentData::ToolUse {
                    id: accumulated.id.unwrap_or_default(),
                    name: tool_name.clone(),
                    server_id: server_id.clone(),
                    input,
                };

                tracing::info!(
                    "MCP: Finalized tool use at index {}: id={}, name={}, server_id={}",
                    index,
                    tool_use.to_message_content().content_type(),
                    tool_name,
                    server_id,
                );

                content_blocks.push((index, tool_use.to_message_content()));
            }
        }

        Ok(content_blocks)
    }
}

#[cfg(test)]
mod tests {
    use super::{build_artifact_download_url, file_download_origin, tool_system_guidance};
    use crate::core::config::CodeSandboxConfig;
    use uuid::Uuid;

    fn tool(name: &str) -> ai_providers::Tool {
        ai_providers::Tool::function(name.to_string(), String::new(), serde_json::json!({}))
    }

    #[test]
    fn guidance_always_includes_tool_preference_nudge() {
        let g = tool_system_guidance(&[]);
        assert!(g.contains("prefer using these tools"), "{g}");
    }

    #[test]
    fn guidance_adds_file_url_rule_only_when_get_resource_link_present() {
        // Absent → no file-URL rule.
        let without = tool_system_guidance(&[tool("abc__some_other_tool")]);
        assert!(!without.contains("get_resource_link"), "{without}");

        // Present (real name shape is "{server_id}__get_resource_link") → rule added.
        let with = tool_system_guidance(&[
            tool("abc__some_other_tool"),
            tool("11111111-2222-3333-4444-555555555555__get_resource_link"),
        ]);
        assert!(with.contains("you MUST first call get_resource_link"), "{with}");
        assert!(with.contains("Never invent, guess, or construct a file/download URL"), "{with}");

        // A different tool merely containing the substring must NOT trigger it
        // (suffix match guards against e.g. "get_resource_link_v2").
        let lookalike = tool_system_guidance(&[tool("abc__get_resource_link_v2")]);
        assert!(!lookalike.contains("you MUST first call get_resource_link"), "{lookalike}");
    }

    fn cs(public_base_url: Option<&str>) -> CodeSandboxConfig {
        CodeSandboxConfig {
            public_base_url: public_base_url.map(str::to_string),
            ..Default::default()
        }
    }

    #[test]
    fn origin_falls_back_to_127_0_0_1_loopback_when_no_public_base_url() {
        // No code_sandbox config at all → loopback. Crucially the loopback is
        // 127.0.0.1, never 0.0.0.0 — file_download_origin never consults
        // server.host, so a 0.0.0.0 bind can't leak into the URL.
        assert_eq!(file_download_origin(None, 8080), "http://127.0.0.1:8080");
        // code_sandbox present but public_base_url unset → still loopback.
        assert_eq!(
            file_download_origin(Some(&cs(None)), 3000),
            "http://127.0.0.1:3000"
        );
    }

    #[test]
    fn origin_uses_public_base_url_when_set() {
        let c = cs(Some("https://tunnel.example.com"));
        assert_eq!(
            file_download_origin(Some(&c), 8080),
            "https://tunnel.example.com"
        );
    }

    #[test]
    fn build_url_trims_trailing_slash_on_api_prefix() {
        // A config value of "/api/" must not produce a double slash.
        let id = Uuid::nil();
        let url = build_artifact_download_url("https://h.example", "/api/", id, "t");
        assert_eq!(
            url,
            format!("https://h.example/api/files/{id}/download-with-token?token=t")
        );
        // Empty prefix is also valid (single leading slash from the literal).
        let url_empty = build_artifact_download_url("https://h.example", "", id, "t");
        assert_eq!(
            url_empty,
            format!("https://h.example/files/{id}/download-with-token?token=t")
        );
    }

    #[test]
    fn build_url_uses_origin_and_preserves_token() {
        let id = Uuid::nil();
        let url = build_artifact_download_url(
            "https://tunnel.example.com",
            "/api",
            id,
            "eyJhbGc.payload.sig-_",
        );
        assert_eq!(
            url,
            format!("https://tunnel.example.com/api/files/{id}/download-with-token?token=eyJhbGc.payload.sig-_")
        );
        // The JWT (with its `.`/`-`/`_` chars) must be preserved byte-for-byte.
        assert!(url.ends_with("?token=eyJhbGc.payload.sig-_"));
    }

    #[test]
    fn end_to_end_artifact_url_never_emits_wildcard_with_public_base_url() {
        // Regression for the reported bug: with public_base_url configured the
        // artifact URL the LLM receives is rooted at the public origin and
        // carries no loopback/wildcard host.
        let c = cs(Some("https://pub.example.com"));
        let origin = file_download_origin(Some(&c), 8080);
        let url = build_artifact_download_url(&origin, "/api", Uuid::nil(), "tok");
        assert!(url.starts_with("https://pub.example.com/api/files/"), "{url}");
        assert!(!url.contains("127.0.0.1"), "{url}");
        assert!(!url.contains("0.0.0.0"), "{url}");
    }

    #[test]
    fn end_to_end_artifact_url_uses_loopback_not_wildcard_without_public_base_url() {
        // Without public_base_url the URL is the 127.0.0.1 loopback (reachable
        // by a same-host MCP server) — and must never be 0.0.0.0.
        let origin = file_download_origin(Some(&cs(None)), 8080);
        let url = build_artifact_download_url(&origin, "/api", Uuid::nil(), "tok");
        assert!(url.starts_with("http://127.0.0.1:8080/api/files/"), "{url}");
        assert!(!url.contains("0.0.0.0"), "{url}");
    }
}

#[cfg(test)]
mod builtin_tests {
    use super::*;
    use serde_json::json;
    use std::collections::HashMap;

    #[test]
    fn side_effect_classification() {
        let memory = crate::modules::memory_mcp::memory_mcp_server_id();
        // Memory built-in remember/forget are the only side-effect tools.
        assert!(is_side_effect_tool(memory, "remember"));
        assert!(is_side_effect_tool(memory, "forget"));
        assert!(!is_side_effect_tool(memory, "recall"));
        assert!(!is_side_effect_tool(memory, "anything_else"));
        // Read tools on the files built-in are NOT side-effect.
        let files = crate::modules::files_mcp::files_mcp_server_id();
        assert!(!is_side_effect_tool(files, "read_file"));
        // A third-party server's "remember" tool must NOT be treated as
        // side-effect — its result may be something the model needs.
        assert!(!is_side_effect_tool(Uuid::new_v4(), "remember"));
        assert!(!is_side_effect_tool(Uuid::new_v4(), "forget"));
    }

    #[test]
    fn auto_attach_ids_from_flags() {
        let elicit = crate::modules::elicitation_mcp::elicitation_mcp_server_id();
        let files = crate::modules::files_mcp::files_mcp_server_id();
        let memory = crate::modules::memory_mcp::memory_mcp_server_id();
        let web = crate::modules::web_search::web_search_server_id();
        let bio = crate::modules::bio_mcp::bio_mcp_server_id();
        let lit = crate::modules::lit_search::lit_search_server_id();
        let tool_result = crate::modules::tool_result_mcp::tool_result_mcp_server_id();

        // Non-tool-capable model (no model_tools_capable seeded) → NOTHING
        // auto-attaches. ask_user must NOT be sent to a model that can't call
        // tools (regression guard: attaching it ran the full MCP body + a tools
        // array on every chat, incl. non-tool-capable / MCP-off chats).
        let mut m: HashMap<String, serde_json::Value> = HashMap::new();
        assert!(auto_attach_builtin_ids(&m).is_empty());
        // Explicit false is the same.
        m.insert("model_tools_capable".into(), json!(false));
        assert!(auto_attach_builtin_ids(&m).is_empty());

        // Tool-capable model → the always-on built-ins (elicitation `ask_user` +
        // `tool_result` `get_tool_result`) are attached even with no flags.
        let always_on = [elicit, tool_result];
        let mut m = HashMap::new();
        m.insert("model_tools_capable".into(), json!(true));
        let base = auto_attach_builtin_ids(&m);
        assert_eq!(base.len(), 2);
        assert!(always_on.iter().all(|id| base.contains(id)));
        // The capability flag round-trips as a "true"/"false" string too.
        let mut ms = HashMap::new();
        ms.insert("model_tools_capable".into(), json!("true"));
        let base_s = auto_attach_builtin_ids(&ms);
        assert_eq!(base_s.len(), 2);
        assert!(always_on.iter().all(|id| base_s.contains(id)));

        // The flag-gated built-ins add on top of the always-on pair.
        m.insert("attach_files_mcp".into(), json!("true"));
        let with_files = auto_attach_builtin_ids(&m);
        assert!(with_files.contains(&files) && with_files.contains(&elicit));
        assert_eq!(with_files.len(), 3);
        m.insert("attach_memory_mcp".into(), json!("true"));
        let all = auto_attach_builtin_ids(&m);
        assert!(all.contains(&files) && all.contains(&memory) && all.contains(&elicit));
        assert_eq!(all.len(), 4);
        // bio attaches on its own flag, on top of the others.
        m.insert("attach_bio_mcp".into(), json!("true"));
        let with_bio = auto_attach_builtin_ids(&m);
        assert!(with_bio.contains(&bio));
        assert_eq!(with_bio.len(), 5);
        // web_search adds on top when its flag is set.
        m.insert("attach_web_search_mcp".into(), json!("true"));
        let with_web = auto_attach_builtin_ids(&m);
        assert!(with_web.contains(&web));
        assert_eq!(with_web.len(), 6);
        // lit_search adds on top when ITS flag is set.
        m.insert(crate::modules::lit_search::chat_extension::ATTACH_FLAG.into(), json!("true"));
        let with_lit = auto_attach_builtin_ids(&m);
        assert!(
            with_lit.contains(&lit)
                && with_lit.contains(&web)
                && with_lit.contains(&bio)
                && with_lit.contains(&files)
                && with_lit.contains(&memory)
                && with_lit.contains(&elicit)
                && with_lit.contains(&tool_result)
        );
        assert_eq!(with_lit.len(), 7);
        // A non-"true" flag value is ignored — only the always-on pair remains.
        let mut m2: HashMap<String, serde_json::Value> = HashMap::new();
        m2.insert("model_tools_capable".into(), json!(true));
        m2.insert("attach_files_mcp".into(), json!("false"));
        let only_base = auto_attach_builtin_ids(&m2);
        assert_eq!(only_base.len(), 2);
        assert!(always_on.iter().all(|id| only_base.contains(id)));
    }

    #[test]
    fn elicitation_is_builtin_and_auto_approved() {
        // ask_user must be treated as a built-in so its tool skips the manual
        // approval prompt (the user answering the form IS the approval).
        assert!(is_builtin_server_id(
            crate::modules::elicitation_mcp::elicitation_mcp_server_id()
        ));
    }

    #[test]
    fn builtin_server_id_recognizes_zero_config_builtins() {
        assert!(is_builtin_server_id(
            crate::modules::files_mcp::files_mcp_server_id()
        ));
        assert!(is_builtin_server_id(
            crate::modules::memory_mcp::memory_mcp_server_id()
        ));
        assert!(is_builtin_server_id(
            crate::modules::elicitation_mcp::elicitation_mcp_server_id()
        ));
        // bio is approval-bypassed too (auto-attached, read-only searches) —
        // even though, unlike the three above, it stays admin-editable.
        assert!(is_builtin_server_id(
            crate::modules::bio_mcp::bio_mcp_server_id()
        ));
        // web_search is approval-bypassed too (auto-attached, read-only).
        assert!(is_builtin_server_id(
            crate::modules::web_search::web_search_server_id()
        ));
        // lit_search (auto-attached, read-only scholarly search/fetch) and
        // tool_result (read-only recall) are approval-bypassed too.
        assert!(is_builtin_server_id(
            crate::modules::lit_search::lit_search_server_id()
        ));
        assert!(is_builtin_server_id(
            crate::modules::tool_result_mcp::tool_result_mcp_server_id()
        ));
        // A third-party server id is NOT a privileged built-in.
        assert!(!is_builtin_server_id(Uuid::new_v4()));
    }
}
