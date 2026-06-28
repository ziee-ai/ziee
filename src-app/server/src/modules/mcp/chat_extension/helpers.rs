// MCP extension helper functions

use axum::response::sse::Event;
use serde_json::Value;
use std::convert::Infallible;
use std::time::Duration;
use uuid::Uuid;

use crate::common::AppError;
use crate::modules::chat::core::types::streaming::SSEChatStreamEvent;
use crate::modules::mcp::client::session::McpSession;
use crate::modules::mcp::client::traits::Tool;
use crate::modules::mcp::{McpRepository, McpServer};

use super::content::McpContentData;
use super::extension::{
    McpServerConfig, SSEChatStreamMcpApprovalRequiredData, SSEChatStreamMcpElicitationRequiredData,
    SSEChatStreamMcpToolCompleteData, SSEChatStreamMcpToolStartData, SSEChatStreamArtifactCreatedData,
};

/// Get all MCP servers accessible to the user
pub async fn get_all_accessible_config(
    pool: &sqlx::PgPool,
    user_id: Uuid,
) -> Result<Vec<McpServer>, AppError> {
    let repo = McpRepository::new(pool.clone());

    // Get all accessible servers (user servers + system servers via groups)
    let response = repo
        .list_accessible(user_id, 1, 1000, None, None, None)
        .await?;

    // Filter out disabled servers
    let enabled_servers: Vec<McpServer> = response
        .servers
        .into_iter()
        .filter(|s| s.enabled)
        .collect();

    Ok(enabled_servers)
}

/// Validate requested servers and build final configuration.
/// Returns (valid_configs, accessible_server_ids, accessible_servers).
/// The full `accessible_servers` list is returned so callers can reuse it
/// instead of re-issuing `get_all_accessible_config` for the same request.
pub async fn validate_and_build_config(
    pool: &sqlx::PgPool,
    user_id: Uuid,
    requested_servers: Option<Vec<McpServerConfig>>,
) -> Result<(Vec<(Uuid, Vec<String>)>, Vec<Uuid>, Vec<McpServer>), AppError> {
    // Get all accessible servers
    let accessible_servers = get_all_accessible_config(pool, user_id).await?;
    let accessible_ids: Vec<Uuid> = accessible_servers.iter().map(|s| s.id).collect();

    let config = if let Some(requested) = requested_servers {
        // Validate each requested server
        let mut valid_configs = Vec::new();

        for req in requested {
            // Check if user has access to this server
            if !accessible_ids.contains(&req.server_id) {
                tracing::warn!(
                    "User {} requested inaccessible MCP server {}",
                    user_id,
                    req.server_id
                );
                continue; // Skip inaccessible servers
            }

            valid_configs.push((req.server_id, req.tools));
        }

        valid_configs
    } else {
        // No specific servers requested - use all accessible servers with all tools
        accessible_ids.iter().map(|&id| (id, vec![])).collect()
    };

    Ok((config, accessible_ids, accessible_servers))
}

/// Anthropic API tool-name regex: `^[a-zA-Z0-9_-]{1,128}$`.
/// Composed names produced by [`convert_mcp_tool_to_ai_tool`] must
/// satisfy this OR they fail silently at chat time with a confusing
/// provider error (closes the latent bug called out by the Phase 8 audit
/// — affects ANY MCP server with oversize or non-conforming names,
/// not just workflow_mcp).
const MAX_ANTHROPIC_TOOL_NAME_LEN: usize = 128;

/// True if `name` is composed entirely of ASCII letters, digits,
/// underscores, or hyphens. Matches Anthropic's tool-name regex
/// character set.
fn is_anthropic_tool_name_charset(name: &str) -> bool {
    !name.is_empty()
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

/// Convert MCP Tool to AI provider Tool format
/// Uses server_id (UUID) to ensure uniqueness across users with same server names.
///
/// Returns `None` when the composed `<server_id>__<tool_name>` would
/// fail Anthropic's `^[a-zA-Z0-9_-]{1,128}$` constraint — either too
/// long, or contains characters outside the safe charset. The caller
/// MUST drop the tool from the list it ships to the LLM in that case
/// (a silent rename would break tool dispatch on the return path).
pub fn convert_mcp_tool_to_ai_tool(
    server_id: Uuid,
    mcp_tool: &Tool,
) -> Option<ai_providers::Tool> {
    // Use double underscore separator for compatibility with Anthropic's naming rules
    // Anthropic requires: ^[a-zA-Z0-9_-]{1,128}$ (no colons allowed)
    // Using server_id (UUID) ensures uniqueness when multiple servers have same name
    let composed = format!("{}__{}", server_id, mcp_tool.name);
    if composed.len() > MAX_ANTHROPIC_TOOL_NAME_LEN {
        tracing::warn!(
            server_id = %server_id,
            tool_name = %mcp_tool.name,
            composed_len = composed.len(),
            cap = MAX_ANTHROPIC_TOOL_NAME_LEN,
            "mcp: dropping tool — composed name exceeds Anthropic's 128-char cap"
        );
        return None;
    }
    if !is_anthropic_tool_name_charset(&composed) {
        tracing::warn!(
            server_id = %server_id,
            tool_name = %mcp_tool.name,
            "mcp: dropping tool — composed name contains characters outside ^[a-zA-Z0-9_-]+$"
        );
        return None;
    }
    Some(ai_providers::Tool::function(
        composed,
        mcp_tool.description.clone().unwrap_or_default(),
        mcp_tool.input_schema.clone(),
    ))
}

/// How long the built-in `ask_user` tool waits for the human to answer before
/// giving up and returning a "no response" result. The intercepted `ask_user`
/// path returns from `execute_tool` BEFORE its outer `timeout_seconds + 300`
/// wrap, so this is the SOLE bound on the form-fill; it's sized to match the
/// ~300s elicitation budget that wrap grants the external-MCP elicitation path.
const ASK_USER_ELICITATION_TIMEOUT: Duration = Duration::from_secs(300);

/// Display name shown in the elicitation form when the ASSISTANT (not a
/// third-party MCP server) is the one asking.
const ASK_USER_SERVER_LABEL: &str = "Assistant";

/// Map an elicitation response (the user's answer, or a synthesized
/// cancel/timeout/stream-closed) to the `(tool_result_text, is_error)` the
/// model receives. Pure + unit-testable.
///
/// EVERY outcome is non-error (`is_error == false`): a decline / cancel /
/// timeout is a legitimate answer the assistant should reason about, not a
/// tool failure it should retry. `accept` returns the answer content as a
/// JSON string so the model can parse the field values.
/// Generous ceiling on the persisted `structuredContent` (stored as JSONB +
/// shipped to the frontend + recalled via `get_tool_result`). Beyond it the
/// typed copy is DROPPED (the readable text digest still works). Fits a
/// max-size literature result of ~200 records.
const MAX_STRUCTURED_CONTENT_BYTES: usize = 1_000_000;

/// Drop a `structuredContent` payload that serializes beyond
/// [`MAX_STRUCTURED_CONTENT_BYTES`] (or that fails to serialize). Returns the
/// payload unchanged when it's within the cap. Production calls this from
/// `execute_tool`; extracted so the cap is unit-testable.
fn cap_structured_content(
    sc: Option<serde_json::Value>,
    tool_name: &str,
) -> Option<serde_json::Value> {
    sc.filter(|sc| {
        let too_big = serde_json::to_string(sc)
            .map(|s| s.len() > MAX_STRUCTURED_CONTENT_BYTES)
            .unwrap_or(true);
        if too_big {
            tracing::warn!(
                "dropping oversized structuredContent (> {} bytes) from tool '{}'",
                MAX_STRUCTURED_CONTENT_BYTES,
                tool_name
            );
        }
        !too_big
    })
}

fn ask_user_tool_result(
    response: &crate::modules::mcp::elicitation::models::ElicitationResponse,
) -> (String, bool) {
    match response.action.as_str() {
        "accept" => {
            let content = response.content.clone().unwrap_or(Value::Null);
            (
                serde_json::to_string(&content).unwrap_or_else(|_| "{}".to_string()),
                false,
            )
        }
        "decline" => ("The user declined to answer.".to_string(), false),
        // cancel / timeout / stream-closed / anything unexpected
        _ => (
            "The user did not respond (cancelled or timed out).".to_string(),
            false,
        ),
    }
}

/// Drive the built-in `ask_user` elicitation INLINE in the chat-stream context.
///
/// Mirrors the external-MCP-server path in `mcp/client/http.rs` (register →
/// `ElicitationStartedNotification` → `mcpElicitationRequired` SSE → block on
/// the oneshot), but returns the user's answer as the tool result instead of
/// POSTing it back to a server. The whole existing pipeline is reused: the
/// global registry, the chat extension's owner-bind + content-block persister
/// (driven by the notification), the FE form, and the
/// `POST /api/mcp/elicitation/{id}/respond` endpoint that unblocks the oneshot.
pub(crate) async fn run_ask_user_elicitation(
    input: Value,
    message_id: Option<uuid::Uuid>,
    owner_user_id: Option<uuid::Uuid>,
    sse_tx: Option<tokio::sync::mpsc::UnboundedSender<Result<Event, Infallible>>>,
    elicit_notify_tx: Option<
        tokio::sync::mpsc::UnboundedSender<
            crate::modules::mcp::elicitation::models::ElicitationStartedNotification,
        >,
    >,
) -> McpContentData {
    use crate::modules::mcp::elicitation::{models, registry};

    // Builds the ToolResult; tool_use_id + server_id are stamped by the caller
    // (same as execute_tool's success path).
    let ask_result = |content: String, is_error: bool| McpContentData::ToolResult {
        tool_use_id: String::new(),
        name: Some("ask_user".to_string()),
        server_id: None,
        content,
        is_error: if is_error { Some(true) } else { None },
        attachment: None,
        images: None,
        resource_links: None,
        hidden_content: None,
        structured_content: None,
    };

    let message = input
        .get("message")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_string();
    if message.is_empty() {
        return ask_result("ask_user requires a non-empty 'message'.".to_string(), true);
    }
    let requested_schema = input
        .get("schema")
        .cloned()
        .unwrap_or_else(|| serde_json::json!({ "type": "object" }));

    // The schema is LLM-generated and arbitrary; the FE renders a form field
    // per property, so a pathologically large/nested schema can hang the
    // browser. Reject anything over the same 1 MB cap used for structured
    // content rather than streaming it to the client. The model gets a clean
    // tool-result error and can retry with a smaller schema.
    let schema_bytes = serde_json::to_vec(&requested_schema)
        .map(|v| v.len())
        .unwrap_or(usize::MAX);
    if schema_bytes > MAX_STRUCTURED_CONTENT_BYTES {
        return ask_result(
            format!(
                "ask_user 'schema' is too large ({schema_bytes} bytes; limit \
                 {MAX_STRUCTURED_CONTENT_BYTES}). Send a smaller schema."
            ),
            true,
        );
    }

    // No interactive stream (e.g. the before_llm_call no-SSE path) → nobody to ask.
    let Some(sse_tx) = sse_tx else {
        return ask_result(
            "The user did not respond (no interactive session available).".to_string(),
            false,
        );
    };

    let elicitation_id = uuid::Uuid::new_v4();
    let content_id = uuid::Uuid::new_v4();
    let (etx, erx) = tokio::sync::oneshot::channel::<models::ElicitationResponse>();
    registry::register(elicitation_id, etx, Some(content_id));

    // Bind the owning user SYNCHRONOUSLY — before the elicitation_id is ever
    // observable on the SSE stream — so a very fast `/respond` can't lose a race
    // with the detached notify-task bind and get a spurious fail-closed 403. The
    // notify task below ALSO binds (idempotent) and is the source of truth for
    // the DB content-block persistence.
    if let Some(uid) = owner_user_id {
        registry::bind_owner(elicitation_id, uid);
    }

    // Persist the pending DB content block + (idempotently) bind the owning user
    // — handled by the chat extension's elicit_notify listener.
    if let Some(ref notify_tx) = elicit_notify_tx {
        let _ = notify_tx.send(models::ElicitationStartedNotification {
            elicitation_id,
            content_id,
            message_id,
            message: message.clone(),
            requested_schema: requested_schema.clone(),
            server: ASK_USER_SERVER_LABEL.to_string(),
        });
    }

    // Surface the form on the chat token stream (same event the FE already
    // renders). Use the TYPED SSEChatStreamEvent variant — like
    // send_tool_start_event — so the serialized payload carries the `type`
    // discriminator the per-user chat stream keys extension events on (a
    // hand-built Event without `type` is silently dropped by consumers).
    let event = SSEChatStreamEvent::McpElicitationRequired(SSEChatStreamMcpElicitationRequiredData {
        elicitation_id: elicitation_id.to_string(),
        message_id: message_id.map(|m| m.to_string()),
        message: message.clone(),
        requested_schema: requested_schema.clone(),
        server: ASK_USER_SERVER_LABEL.to_string(),
    });
    if sse_tx.send(Ok(event.into())).is_err() {
        let _ = registry::remove(elicitation_id);
        return ask_result(
            "The user did not respond (the chat stream closed).".to_string(),
            false,
        );
    }

    // Block until the user answers, hits Stop (stream closes), or we time out.
    let response = tokio::select! {
        r = erx => r.unwrap_or(models::ElicitationResponse {
            action: "cancel".to_string(),
            content: None,
        }),
        _ = sse_tx.closed() => {
            let _ = registry::remove(elicitation_id);
            models::ElicitationResponse { action: "cancel".to_string(), content: None }
        }
        _ = tokio::time::sleep(ASK_USER_ELICITATION_TIMEOUT) => {
            let _ = registry::remove(elicitation_id);
            models::ElicitationResponse { action: "cancel".to_string(), content: None }
        }
    };

    let (content, is_error) = ask_user_tool_result(&response);
    ask_result(content, is_error)
}

/// Execute a tool via MCP session
///
/// # Arguments
/// * `session` - MCP session
/// * `tool_name` - Clean tool name (without server_id prefix)
/// * `input` - Tool input parameters
/// * `_server_name` - Server name (for logging)
/// * `timeout_seconds` - Execution timeout
pub async fn execute_tool(
    session: &mut McpSession,
    tool_name: &str,
    input: Value,
    _server_name: &str,
    timeout_seconds: Option<i32>,
    message_id: Option<uuid::Uuid>,
    sse_tx: Option<tokio::sync::mpsc::UnboundedSender<Result<Event, Infallible>>>,
    elicit_notify_tx: Option<tokio::sync::mpsc::UnboundedSender<crate::modules::mcp::elicitation::models::ElicitationStartedNotification>>,
) -> (McpContentData, bool) {
    // Returns (result, user_only_audience).
    //
    // user_only_audience is true when at least one content block carries the
    // MCP-spec `annotations.audience: ["user"]` metadata EXACTLY — meaning
    // "intended for the human user only, not the assistant." When set, the
    // caller streams the tool text directly to the user without another LLM
    // call (the tool's output IS the assistant's final answer).
    //
    // The exact-match check (audience contains "user" and ONLY "user", not
    // also "assistant") is deliberate: per the MCP spec
    // (modelcontextprotocol.io/specification/2025-11-25/server/resources#annotations),
    // `["user", "assistant"]` means "both audiences should see it" — the LLM
    // should ALSO process such content, which means we must NOT bypass it.

    // `ask_user` (the built-in elicitation tool) is driven INLINE here instead
    // of being dispatched over the loopback: only this chat-stream context holds
    // the live `sse_tx` needed to surface the form. It blocks until the user
    // answers and returns their answer as the tool result.
    if tool_name == "ask_user"
        && session.server_id()
            == crate::modules::elicitation_mcp::elicitation_mcp_server_id()
    {
        // Defensive fallback path (sampling / before_llm_call approved-tools):
        // no user_id in scope here, so the owning user is bound by the notify
        // task. The hot path (after_llm_call) binds synchronously — see mcp.rs.
        let result =
            run_ask_user_elicitation(input, message_id, None, sse_tx, elicit_notify_tx).await;
        return (result, false);
    }

    // Elicitation may block for up to 300s; use a generous outer timeout so that
    // elicitation requests have time to complete before we give up.
    // The tool-level timeout is enforced separately inside call_tool_with_sampling.
    let timeout_secs = timeout_seconds.unwrap_or(30) as u64 + 300;
    let timeout = Duration::from_secs(timeout_secs);

    let result = tokio::time::timeout(
        timeout,
        session.call_tool(tool_name, input.clone(), message_id, sse_tx, elicit_notify_tx)
    ).await;

    match result {
        Ok(Ok(tool_result)) => {
            // Success - convert MCP ToolResult to our format, parsing rich content types
            // Cap on inline base64 size for tool-returned files/images. Without it a
            // malicious or buggy MCP tool could return a huge blob that blows up
            // memory, the DB row, the request, and prompt-cache write cost. ~6 MB
            // decoded (8M base64 chars).
            const MAX_INLINE_TOOL_FILE_B64: usize = 8_000_000;
            // Aggregate bounds so many sub-cap images can't add up to an unbounded
            // request/DB row.
            const MAX_IMAGES: usize = 8;
            const MAX_TOTAL_IMAGE_B64: usize = 24_000_000;
            let mut text_parts: Vec<String> = Vec::new();
            let mut attachment: Option<super::content::RichFile> = None;
            let mut images: Vec<super::content::RichFile> = Vec::new();
            let mut total_image_b64: usize = 0;
            let mut resource_links: Vec<super::content::ResourceLink> = Vec::new();

            for item in &tool_result.content {
                let content_type = item.content.get("type").and_then(|t| t.as_str()).unwrap_or("text");
                match content_type {
                    "text" => {
                        if let Some(text) = item.content.get("text").and_then(|t| t.as_str()) {
                            text_parts.push(text.to_string());
                        }
                    }
                    "file" => {
                        // First file wins (single attachment slot).
                        if attachment.is_none() {
                            if let (Some(filename), Some(mime_type), Some(data)) = (
                                item.content.get("filename").and_then(|v| v.as_str()),
                                item.content.get("mime_type").and_then(|v| v.as_str()),
                                item.content.get("data").and_then(|v| v.as_str()),
                            ) {
                                if data.len() <= MAX_INLINE_TOOL_FILE_B64 {
                                    attachment = Some(super::content::RichFile {
                                        filename: filename.to_string(),
                                        mime_type: mime_type.to_string(),
                                        data: data.to_string(),
                                    });
                                } else {
                                    tracing::warn!(
                                        "mcp: dropping oversized tool file '{}' ({} base64 bytes)",
                                        filename,
                                        data.len()
                                    );
                                }
                            }
                        }
                    }
                    "resource_link" => {
                        // MCP resource_link: a reference to a persisted resource (not inline content)
                        if let Some(link) =
                            crate::modules::mcp::resource_link::parse_resource_link_block(&item.content)
                        {
                            let name = link.name.clone().unwrap_or_else(|| "file".to_string());
                            // Guard #3 (defense in depth): never echo a raw `ziee://` host
                            // path into the LLM-facing confirmation. On the happy path the
                            // tool-result content is overwritten after the save pipeline
                            // (mcp::resource_link::persist_links + the artifact-info rewrite);
                            // this placeholder also covers the save-failure path.
                            let uri_for_text =
                                if crate::modules::mcp::resource_link::is_ziee_host_path(&link.uri) {
                                    "(saved server-side; appears as a file attachment)".to_string()
                                } else {
                                    link.uri.clone()
                                };
                            resource_links.push(link);
                            // Provide the LLM with a readable confirmation so it doesn't retry
                            text_parts.push(format!(
                                "resource_link available — name: {}, uri: {}",
                                name, uri_for_text
                            ));
                        }
                    }
                    "image" => {
                        // MCP ImageContent: base64 `data` + `mimeType`. Capture ALL
                        // images (replayed to the model as image blocks by
                        // content::to_content_block), each bounded by the size cap.
                        if let (Some(data), Some(mime_type)) = (
                            item.content.get("data").and_then(|v| v.as_str()),
                            item.content.get("mimeType").and_then(|v| v.as_str()),
                        ) {
                            if mime_type.starts_with("image/") {
                                if data.len() <= MAX_INLINE_TOOL_FILE_B64
                                    && images.len() < MAX_IMAGES
                                    && total_image_b64 + data.len() <= MAX_TOTAL_IMAGE_B64
                                {
                                    total_image_b64 += data.len();
                                    let ext = mime_type.rsplit('/').next().unwrap_or("png");
                                    images.push(super::content::RichFile {
                                        filename: format!("tool-image.{ext}"),
                                        mime_type: mime_type.to_string(),
                                        data: data.to_string(),
                                    });
                                } else {
                                    tracing::warn!(
                                        "mcp: dropping oversized tool image ({} base64 bytes)",
                                        data.len()
                                    );
                                }
                            }
                        }
                    }
                    _ => {
                        // Unknown type: serialize as-is
                        if let Ok(s) = serde_json::to_string(&item.content) {
                            text_parts.push(s);
                        }
                    }
                }
            }

            let content_text = text_parts.join("\n");

            // Truncate if too large (100KB limit). Walk back to the
            // nearest UTF-8 char boundary so we never split a
            // multi-byte sequence — closes 04-chat F-12 (Low).
            let final_content = if content_text.len() > 100_000 {
                let mut cut = 100_000;
                while cut > 0 && !content_text.is_char_boundary(cut) {
                    cut -= 1;
                }
                format!(
                    "{}\n\n[... truncated {} bytes ...]",
                    &content_text[..cut],
                    content_text.len() - cut
                )
            } else {
                content_text
            };

            // Bound the persisted structuredContent the same way `content` is
            // bounded above: it's stored as JSONB + shipped to the frontend, so a
            // pathologically large tool payload must not bloat the row/response
            // unboundedly. Generous ceiling (fits a max-size literature result of
            // ~200 records); beyond it we DROP it (None) — the readable text
            // digest still works, only the typed UI copy degrades.
            let mut structured_content =
                cap_structured_content(tool_result.structured_content.clone(), tool_name);

            // Guard #3 (defense in depth): a raw `ziee://<host_path>` must never persist into
            // the tool result the browser reads / `get_tool_result` recalls.
            //   - `structured_content` is display/recall-only (never used to ingest), so scrub
            //     it unconditionally here — this closes the `get_resource_link` →
            //     `structuredContent` host-path disclosure.
            //   - `resource_links` carry the raw `ziee://` that `persist_links` needs to
            //     INGEST, so they're rewritten/blanked there on the normal path. But
            //     `persist_links` is skipped for ERROR results, so blank any leftover
            //     `ziee://` link here when the tool errored (the file was never produced).
            if let Some(sc) = structured_content.as_mut() {
                crate::modules::mcp::resource_link::scrub_ziee_in_value(sc);
            }
            if tool_result.is_error {
                for l in resource_links.iter_mut() {
                    if crate::modules::mcp::resource_link::is_ziee_host_path(&l.uri) {
                        l.uri = String::new();
                    }
                }
            }

            let mcp_result = McpContentData::ToolResult {
                tool_use_id: String::new(), // Will be set by caller
                name: Some(tool_name.to_string()),
                server_id: None, // Will be set by caller
                content: final_content,
                is_error: Some(tool_result.is_error),
                attachment,
                images: if images.is_empty() { None } else { Some(images) },
                resource_links: if resource_links.is_empty() { None } else { Some(resource_links) },
                hidden_content: None, // Set later if resource_links artifacts are saved
                // Persist the tool response's structuredContent (UI render +
                // get_tool_result recall; not forwarded to the LLM by
                // to_content_block). Size-capped just above.
                structured_content,
            };
            // Bypass the LLM only when at least one content block is exactly
            // user-targeted: audience == ["user"] (single-element array, no
            // "assistant"). Per the MCP spec, ["user", "assistant"] means
            // both should see it — the LLM still needs to process the content
            // in that case, so we must NOT bypass.
            let user_only_audience = tool_result.content.iter().any(|c| {
                c.content
                    .get("annotations")
                    .and_then(|a| a.get("audience"))
                    .and_then(|v| v.as_array())
                    .map(|arr| arr.len() == 1 && arr[0].as_str() == Some("user"))
                    .unwrap_or(false)
            });
            (mcp_result, user_only_audience)
        }
        Ok(Err(e)) => {
            // MCP error
            (McpContentData::ToolResult {
                tool_use_id: String::new(),
                name: Some(tool_name.to_string()),
                server_id: None, // Will be set by caller
                content: format!("Tool execution failed: {}", e),
                is_error: Some(true),
                attachment: None,
                images: None,
                resource_links: None,
                hidden_content: None,
                structured_content: None,
            }, false)
        }
        Err(_) => {
            // Timeout
            (McpContentData::ToolResult {
                tool_use_id: String::new(),
                name: Some(tool_name.to_string()),
                server_id: None, // Will be set by caller
                content: format!(
                    "Tool execution timed out after {}s",
                    timeout_secs
                ),
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

/// Send SSE event for tool start.
/// Fire-and-forget: logs a warning if the channel is closed but never fails the caller.
pub async fn send_tool_start_event(
    tx: Option<&tokio::sync::mpsc::UnboundedSender<Result<Event, Infallible>>>,
    tool_use_id: &str,
    tool_name: &str,
    server: &str,
    input: &serde_json::Value,
) {
    if let Some(tx) = tx {
        let event = SSEChatStreamEvent::McpToolStart(SSEChatStreamMcpToolStartData {
            tool_use_id: tool_use_id.to_string(),
            tool_name: tool_name.to_string(),
            server: server.to_string(),
            input: input.clone(),
        });

        if let Err(e) = tx.send(Ok(event.into())) {
            tracing::warn!("Failed to send SSE tool start event: {:?}", e);
        }
    }
}

/// Send SSE event for tool complete.
/// Fire-and-forget: logs a warning if the channel is closed but never fails the caller.
pub async fn send_tool_complete_event(
    tx: Option<&tokio::sync::mpsc::UnboundedSender<Result<Event, Infallible>>>,
    tool_use_id: &str,
    tool_name: &str,
    server: &str,
    is_error: bool,
    result: Option<&str>,
) {
    if let Some(tx) = tx {
        let result_truncated = result.map(|r| {
            if r.len() > 2000 {
                format!("{}...[truncated]", &r[..2000])
            } else {
                r.to_string()
            }
        });

        let event = SSEChatStreamEvent::McpToolComplete(SSEChatStreamMcpToolCompleteData {
            tool_use_id: tool_use_id.to_string(),
            tool_name: tool_name.to_string(),
            server: server.to_string(),
            is_error,
            result: result_truncated,
        });

        if let Err(e) = tx.send(Ok(event.into())) {
            tracing::warn!("Failed to send SSE tool complete event: {:?}", e);
        }
    }
}

/// Send SSE event for approval required.
///
/// This is **fatal** (returns `Err`) if the channel send fails. Unlike tool start/complete
/// events which are purely informational, an approval-required notification that never
/// reaches the client leaves the user with no way to act — there is no point continuing
/// the request.
pub async fn send_approval_required_event(
    tx: Option<&tokio::sync::mpsc::UnboundedSender<Result<Event, Infallible>>>,
    tool_use_id: &str,
    tool_name: &str,
    server: &str,
    server_id: &str,
    input: &serde_json::Value,
) -> Result<(), AppError> {
    if let Some(tx) = tx {
        let event = SSEChatStreamEvent::McpApprovalRequired(SSEChatStreamMcpApprovalRequiredData {
            tool_use_id: tool_use_id.to_string(),
            tool_name: tool_name.to_string(),
            server: server.to_string(),
            server_id: server_id.to_string(),
            input: input.clone(),
        });

        tx.send(Ok(event.into()))
            .map_err(|_| AppError::internal_error("Failed to send SSE event"))?;
    }

    Ok(())
}

/// Send SSE event when a tool creates an artifact file (via MCP resource_link).
/// Fire-and-forget: logs a warning if the channel is closed but never fails the caller.
pub async fn send_artifact_created_event(
    tx: Option<&tokio::sync::mpsc::UnboundedSender<Result<Event, Infallible>>>,
    tool_use_id: &str,
    file_id: &str,
    filename: &str,
    mime_type: Option<&str>,
    file_size: i64,
) {
    if let Some(tx) = tx {
        let event = SSEChatStreamEvent::ArtifactCreated(SSEChatStreamArtifactCreatedData {
            tool_use_id: tool_use_id.to_string(),
            file_id: file_id.to_string(),
            filename: filename.to_string(),
            mime_type: mime_type.map(String::from),
            file_size,
        });

        if let Err(e) = tx.send(Ok(event.into())) {
            tracing::warn!("Failed to send SSE artifact created event: {:?}", e);
        }
    }
}

/// Build tool input by mapping user message text to the first required string parameter.
///
/// Returns `None` when the schema has required parameters but none of them are strings,
/// meaning we cannot auto-map the query text — the caller should skip "always mode" for
/// this tool rather than submitting wrong inputs silently.
///
/// Returns `Some` in two cases:
/// - A required string parameter was found → `{ param_name: query_text }`
/// - No schema information available → generic fallback `{ "query": query_text }`
pub fn build_query_input(schema: &serde_json::Value, query_text: &str) -> Option<serde_json::Value> {
    if let (Some(props), Some(required)) = (
        schema.get("properties").and_then(|p| p.as_object()),
        schema.get("required").and_then(|r| r.as_array()),
    ) {
        for req_key in required {
            if let Some(key) = req_key.as_str() {
                let is_string = props.get(key)
                    .and_then(|p| p.get("type"))
                    .and_then(|t| t.as_str())
                    == Some("string");
                if is_string {
                    return Some(serde_json::json!({ key: query_text }));
                }
            }
        }
        // Has required params but none are strings — cannot auto-map
        None
    } else {
        // No schema info — use generic fallback
        Some(serde_json::json!({ "query": query_text }))
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ask_user_tool_result, build_query_input, cap_structured_content,
        convert_mcp_tool_to_ai_tool, run_ask_user_elicitation, McpContentData,
        MAX_ANTHROPIC_TOOL_NAME_LEN, MAX_STRUCTURED_CONTENT_BYTES,
    };
    use crate::modules::mcp::client::traits::Tool as McpToolDef;
    use crate::modules::mcp::elicitation::models::ElicitationResponse;
    use uuid::Uuid;

    /// Pull `(content, is_error)` out of a `ToolResult` for assertions.
    fn tool_result_parts(r: &McpContentData) -> (String, bool) {
        match r {
            McpContentData::ToolResult { content, is_error, .. } => {
                (content.clone(), is_error.unwrap_or(false))
            }
            other => panic!("expected ToolResult, got {other:?}"),
        }
    }

    /// A within-cap structuredContent (e.g. a normal web_search result) is
    /// preserved verbatim.
    #[test]
    fn structured_content_under_cap_is_kept() {
        let sc = serde_json::json!({
            "provider": "searxng",
            "results": [
                { "title": "Rust", "url": "https://rust-lang.org", "snippet": "systems lang" },
                { "title": "Tokio", "url": "https://tokio.rs", "snippet": "async runtime" },
            ],
        });
        let out = cap_structured_content(Some(sc.clone()), "web_search");
        assert_eq!(out, Some(sc), "small payload must pass through unchanged");
    }

    /// An oversized structuredContent (a pathologically large search/fetch
    /// result) is DROPPED to None so it can't bloat the JSONB row / response.
    #[test]
    fn structured_content_over_cap_is_dropped() {
        // Build a results array whose serialized form clears the 1MB ceiling.
        let big_snippet = "x".repeat(2048);
        let results: Vec<_> = (0..1000)
            .map(|i| {
                serde_json::json!({
                    "title": format!("result {i}"),
                    "url": format!("https://example.com/{i}"),
                    "snippet": big_snippet,
                })
            })
            .collect();
        let sc = serde_json::json!({ "provider": "searxng", "results": results });
        assert!(
            serde_json::to_string(&sc).unwrap().len() > MAX_STRUCTURED_CONTENT_BYTES,
            "fixture must actually exceed the cap",
        );
        let out = cap_structured_content(Some(sc), "web_search");
        assert!(out.is_none(), "oversized structuredContent must be dropped");
    }

    /// An empty `message` is a malformed tool call from the model → the ONE
    /// genuine error outcome (so the model retries with a real prompt). Returns
    /// before any registry/SSE work, so it's drivable with all-None args.
    #[tokio::test]
    async fn ask_user_empty_message_is_error() {
        let result = run_ask_user_elicitation(
            serde_json::json!({ "message": "   ", "schema": { "type": "object" } }),
            None,
            None,
            None,
            None,
        )
        .await;
        let (content, is_error) = tool_result_parts(&result);
        assert!(is_error, "empty message must be a tool error");
        assert!(content.contains("non-empty"), "got: {content}");
    }

    /// With no interactive stream (sse_tx == None — the before_llm_call no-SSE
    /// approved-tools path) there's nobody to ask, so ask_user returns a
    /// NON-error "no interactive session" marker (not a failure to retry).
    #[tokio::test]
    async fn ask_user_without_sse_returns_non_error_no_session_marker() {
        let result = run_ask_user_elicitation(
            serde_json::json!({ "message": "Pick a color", "schema": { "type": "object" } }),
            None,
            None,
            None, // no sse_tx
            None,
        )
        .await;
        let (content, is_error) = tool_result_parts(&result);
        assert!(!is_error, "no-session is not a tool failure");
        assert!(content.contains("no interactive session"), "got: {content}");
    }

    /// A pathologically large LLM-generated `schema` is rejected as a tool
    /// error BEFORE it can be streamed to the form (the FE renders a field per
    /// property, so an oversized schema would hang the browser). The size guard
    /// runs ahead of the interactive-stream check, so all-None args drive it.
    #[tokio::test]
    async fn ask_user_oversized_schema_is_error() {
        // Build a JSON-schema object whose serialized form clears the cap.
        let big: std::collections::BTreeMap<String, serde_json::Value> = (0..60_000)
            .map(|i| (format!("field_{i}"), serde_json::json!({ "type": "string" })))
            .collect();
        let schema = serde_json::json!({ "type": "object", "properties": big });
        assert!(
            serde_json::to_vec(&schema).unwrap().len() > MAX_STRUCTURED_CONTENT_BYTES,
            "fixture must actually exceed the cap",
        );
        let result = run_ask_user_elicitation(
            serde_json::json!({ "message": "Pick", "schema": schema }),
            None,
            None,
            None,
            None,
        )
        .await;
        let (content, is_error) = tool_result_parts(&result);
        assert!(is_error, "oversized schema must be a tool error");
        assert!(content.contains("too large"), "got: {content}");
    }

    // ── ask_user response → tool_result mapping (plan Tier 1) ─────────────────

    #[test]
    fn ask_user_accept_returns_answer_json_non_error() {
        let r = ElicitationResponse {
            action: "accept".to_string(),
            content: Some(serde_json::json!({ "color": "green" })),
        };
        let (content, is_error) = ask_user_tool_result(&r);
        assert_eq!(content, r#"{"color":"green"}"#);
        assert!(!is_error, "accept must never be a tool error");
    }

    #[test]
    fn ask_user_accept_without_content_is_json_null() {
        let r = ElicitationResponse {
            action: "accept".to_string(),
            content: None,
        };
        let (content, is_error) = ask_user_tool_result(&r);
        assert_eq!(content, "null");
        assert!(!is_error);
    }

    #[test]
    fn ask_user_decline_returns_marker_non_error() {
        let r = ElicitationResponse {
            action: "decline".to_string(),
            content: None,
        };
        let (content, is_error) = ask_user_tool_result(&r);
        assert!(content.contains("declined"), "got: {content}");
        assert!(!is_error, "decline is an answer, not a failure");
    }

    #[test]
    fn ask_user_cancel_timeout_and_unknown_map_to_no_response_marker() {
        // cancel (explicit), the synthesized timeout/stream-closed cancel, and
        // any unexpected action all collapse to the same non-error "no response"
        // marker so the assistant reasons about it instead of retrying.
        for action in ["cancel", "timeout", "weird-action"] {
            let r = ElicitationResponse {
                action: action.to_string(),
                content: None,
            };
            let (content, is_error) = ask_user_tool_result(&r);
            assert!(
                content.contains("did not respond"),
                "action={action} got: {content}"
            );
            assert!(!is_error, "action={action} must be non-error");
        }
    }

    /// Stream-close DURING the wait: the form is surfaced on the SSE stream,
    /// then the user closes the chat stream (Stop) before answering. The
    /// `sse_tx.closed()` arm of the select must fire and produce a NON-error
    /// "did not respond" marker (so the assistant reasons about it, never
    /// retries). Distinct from the no-SSE path and from the send-time close.
    #[tokio::test]
    async fn ask_user_stream_close_during_wait_returns_non_error_no_response() {
        use tokio::sync::mpsc;
        let (tx, mut rx) =
            mpsc::unbounded_channel::<Result<axum::response::sse::Event, std::convert::Infallible>>();

        let handle = tokio::spawn(run_ask_user_elicitation(
            serde_json::json!({ "message": "Pick a color", "schema": { "type": "object" } }),
            None,
            None,
            Some(tx),
            None,
        ));

        // Receive the elicitation form first — proves the form was surfaced and
        // the elicitation is now blocked on the select — THEN drop the receiver
        // to simulate the chat stream closing before the user answers.
        let _form = rx.recv().await.expect("elicitation form event surfaced");
        drop(rx);

        let result = handle.await.expect("elicitation task joins");
        let (content, is_error) = tool_result_parts(&result);
        assert!(!is_error, "stream-close mid-wait is not a tool failure");
        assert!(
            content.contains("did not respond"),
            "stream-close must map to the no-response marker; got: {content}"
        );
    }

    fn make_mcp_tool(name: &str) -> McpToolDef {
        McpToolDef {
            name: name.to_string(),
            description: Some("test".to_string()),
            input_schema: serde_json::json!({}),
        }
    }

    #[test]
    fn convert_mcp_tool_accepts_safe_name() {
        let server_id = Uuid::new_v4();
        let tool = make_mcp_tool("short_name-1");
        let out = convert_mcp_tool_to_ai_tool(server_id, &tool);
        assert!(out.is_some(), "safe name should produce a tool");
    }

    #[test]
    fn convert_mcp_tool_drops_oversize_composed_name() {
        let server_id = Uuid::new_v4();
        // server_id is 36 chars + "__" = 38; budget for tool_name is 90.
        // Pick > 90 to exceed 128.
        let big = "a".repeat(MAX_ANTHROPIC_TOOL_NAME_LEN);
        let tool = make_mcp_tool(&big);
        let out = convert_mcp_tool_to_ai_tool(server_id, &tool);
        assert!(out.is_none(), "oversize composed name should be dropped");
    }

    #[test]
    fn convert_mcp_tool_drops_disallowed_charset() {
        let server_id = Uuid::new_v4();
        // Colons + dots are common in non-conforming MCP servers and
        // fail Anthropic's regex.
        let tool = make_mcp_tool("category:subtool.v2");
        let out = convert_mcp_tool_to_ai_tool(server_id, &tool);
        assert!(
            out.is_none(),
            "name with colons/dots should be dropped (charset rejection)"
        );
    }

    #[test]
    fn test_build_query_input_required_string_param() {
        let schema = serde_json::json!({
            "required": ["query"],
            "properties": {
                "query": { "type": "string" }
            }
        });
        let result = build_query_input(&schema, "test message");
        assert_eq!(result, Some(serde_json::json!({ "query": "test message" })));
    }

    #[test]
    fn test_build_query_input_fallback_to_query_key() {
        // Schema has no required params (only optional) → uses generic fallback
        let schema = serde_json::json!({
            "properties": {
                "count": { "type": "integer" }
            }
        });
        let result = build_query_input(&schema, "test message");
        assert_eq!(result, Some(serde_json::json!({ "query": "test message" })));
    }

    #[test]
    fn test_build_query_input_picks_first_required_string() {
        // First required string param is "topic", not "limit" (integer)
        let schema = serde_json::json!({
            "required": ["topic", "limit"],
            "properties": {
                "topic": { "type": "string" },
                "limit": { "type": "integer" }
            }
        });
        let result = build_query_input(&schema, "test message");
        assert_eq!(result, Some(serde_json::json!({ "topic": "test message" })));
    }

    #[test]
    fn test_build_query_input_returns_none_for_non_string_required_params() {
        // Schema has required params but none are strings — auto-mapping is impossible
        let schema = serde_json::json!({
            "required": ["count", "enabled"],
            "properties": {
                "count": { "type": "integer" },
                "enabled": { "type": "boolean" }
            }
        });
        let result = build_query_input(&schema, "test message");
        assert_eq!(result, None, "Should return None when required params exist but none are strings");
    }
}
