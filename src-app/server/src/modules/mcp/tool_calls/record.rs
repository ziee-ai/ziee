//! Pure helpers that turn a finished tool call into a `CreateMcpToolCall` row.
//!
//! Kept free of DB / IO so they're directly unit-testable (Tier 1).

use serde_json::{Value, json};
use uuid::Uuid;

use crate::common::AppError;
use crate::modules::mcp::client::ToolResult;

use super::models::{CreateMcpToolCall, McpCallContext, McpToolCallStatus};

/// Max serialized size of stored `arguments_json` before it's replaced with a
/// truncation marker (a giant tool input must not bloat the row).
pub const MAX_ARGS_BYTES: usize = 16 * 1024;

/// Max serialized size of stored `result_json` before it's replaced with a
/// truncation marker. Mirrors the chat path's `structuredContent` cap
/// (`mcp/chat_extension/helpers.rs`) so a huge tool result can't bloat the row
/// or the long-retention history.
pub const MAX_RESULT_BYTES: usize = 1024 * 1024;

/// Content-block fields that carry inline base64 bytes; stripped from the
/// stored result (the bytes already live in the file store via the
/// resource-link pipeline). `data` = MCP file/image/audio content; `blob` =
/// embedded resource.
const STRIPPED_BYTE_KEYS: &[&str] = &["data", "blob"];

/// MCP content-block `type` values that carry inline bytes under `data`/`blob`.
/// We strip those keys ONLY on blocks of these types, so a legitimate short
/// `{"data": "OK"}` elsewhere in a result (e.g. inside `structuredContent`) is
/// preserved verbatim.
const BINARY_BLOCK_TYPES: &[&str] = &["image", "audio", "resource", "file"];

/// Best-effort secret-key denylist (exact, case-insensitive). Values under
/// these keys in args/results are redacted before storage — defense-in-depth
/// against a tool whose PAYLOAD carries a credential landing in the
/// long-retention history. (Request headers / the injected internal JWT are
/// never part of args/results, so this only guards tool-payload secrets.)
const SECRET_KEYS: &[&str] = &[
    "authorization",
    "auth",
    "bearer",
    "password",
    "passwd",
    "secret",
    "token",
    "access_token",
    "refresh_token",
    "api_key",
    "apikey",
    "api-key",
    "x-api-key",
    "client_secret",
    "private_key",
];

fn is_secret_key(key: &str) -> bool {
    let k = key.to_ascii_lowercase();
    SECRET_KEYS.contains(&k.as_str())
}

/// Sanitize a JSON value for storage: redact secret-keyed values, and strip
/// inline base64 bytes (`data`/`blob`) within recognized binary content blocks.
/// Everything else passes through unchanged.
fn sanitize_value(value: Value) -> Value {
    sanitize_inner(value, false)
}

/// `in_binary_block` propagates DOWN through a content block whose `type` is in
/// `BINARY_BLOCK_TYPES`, so inline bytes are stripped even when nested — e.g. an
/// embedded `resource` block keeps its base64 at `resource.blob`, one level
/// below the block's `type`. Bytes under any depth of such a block are stripped;
/// outside one (e.g. a short `{"data":"OK"}` in `structuredContent`) they're not.
fn sanitize_inner(value: Value, in_binary_block: bool) -> Value {
    match value {
        Value::Object(map) => {
            let is_binary_block = in_binary_block
                || map
                    .get("type")
                    .and_then(|t| t.as_str())
                    .map(|t| BINARY_BLOCK_TYPES.contains(&t))
                    .unwrap_or(false);
            Value::Object(
                map.into_iter()
                    .map(|(k, v)| {
                        if is_secret_key(&k) {
                            return (k, json!("[redacted]"));
                        }
                        if is_binary_block && STRIPPED_BYTE_KEYS.contains(&k.as_str()) {
                            if let Value::String(s) = &v {
                                return (k, json!({ "_stripped": true, "_bytes": s.len() }));
                            }
                        }
                        (k, sanitize_inner(v, is_binary_block))
                    })
                    .collect(),
            )
        }
        Value::Array(arr) => {
            Value::Array(arr.into_iter().map(|v| sanitize_inner(v, in_binary_block)).collect())
        }
        other => other,
    }
}

/// Cap a tool's arguments for storage: redact secret-keyed values, then if the
/// serialized form exceeds `MAX_ARGS_BYTES`, store a marker instead.
pub fn cap_arguments(args: &Value) -> Value {
    let redacted = sanitize_value(args.clone());
    let bytes = serde_json::to_string(&redacted).map(|s| s.len()).unwrap_or(0);
    if bytes > MAX_ARGS_BYTES {
        json!({ "_truncated": true, "_bytes": bytes })
    } else {
        redacted
    }
}

/// The capture computed from a successful `ToolResult`.
pub struct ResultCapture {
    /// Full result JSON with base64 bytes stripped to references.
    pub result_json: Value,
    /// Distinct content-block `type` values, in first-seen order.
    pub content_kinds: Vec<String>,
    /// PRE-strip serialized size of the whole result, for transparency.
    pub result_bytes: i64,
}

/// Build the stored-result capture: serialize the whole `ToolResult`, record
/// its pre-strip size + distinct content kinds, then strip base64 bytes.
pub fn capture_result(result: &ToolResult) -> ResultCapture {
    let full = serde_json::to_value(result).unwrap_or(Value::Null);
    let result_bytes = serde_json::to_string(&full)
        .map(|s| s.len() as i64)
        .unwrap_or(0);

    let mut content_kinds: Vec<String> = Vec::new();
    for block in &result.content {
        if let Some(t) = block.content.get("type").and_then(|t| t.as_str()) {
            if !content_kinds.iter().any(|k| k == t) {
                content_kinds.push(t.to_string());
            }
        }
    }

    // Redact secrets + strip inline bytes, then cap the overall size so a huge
    // tool result can't bloat the row (mirrors the chat path's result caps).
    let mut sanitized = sanitize_value(full);
    // Guard #3 (resource_link `ziee://`): this capture runs BEFORE the chat path's scrub
    // (`helpers::execute_tool`), so blank any host-path `ziee://` string here — otherwise a
    // raw host filesystem path (e.g. from `code_sandbox::get_resource_link`) would be
    // persisted into the queryable tool-call history. Only absolute host paths are blanked;
    // `ziee://workflow-runs/...` resource handles are preserved.
    crate::modules::mcp::resource_link::scrub_ziee_in_value(&mut sanitized);
    let result_json = match serde_json::to_string(&sanitized) {
        Ok(s) if s.len() > MAX_RESULT_BYTES => json!({ "_truncated": true, "_bytes": s.len() }),
        _ => sanitized,
    };

    ResultCapture {
        result_json,
        content_kinds,
        result_bytes,
    }
}

/// Classify a transport-level error into a terminal status. Best-effort:
/// timeouts are common enough to break out for the UI.
fn classify_error_status(err: &AppError) -> McpToolCallStatus {
    let code = err.error_code().to_ascii_lowercase();
    let msg = err.to_string().to_ascii_lowercase();
    if code.contains("timeout") || msg.contains("timed out") || msg.contains("timeout") {
        McpToolCallStatus::Timeout
    } else {
        McpToolCallStatus::Failed
    }
}

/// Build the insert payload from a finished tool call. Returns `None` when the
/// session carries no owner (`ctx.user_id` is `None`) — an unstamped session,
/// for which we deliberately record nothing rather than insert a null owner.
#[allow(clippy::too_many_arguments)]
pub fn build_record(
    server_id: Uuid,
    ctx: &McpCallContext,
    tool_name: &str,
    arguments: &Value,
    outcome: &Result<ToolResult, AppError>,
    started_at: time::OffsetDateTime,
    elapsed_ms: i64,
) -> Option<CreateMcpToolCall> {
    let user_id = ctx.user_id?;

    let (status, is_error, result_json, content_kinds, result_bytes, error_message) = match outcome {
        Ok(tr) => {
            let cap = capture_result(tr);
            let status = if tr.is_error {
                McpToolCallStatus::Failed
            } else {
                McpToolCallStatus::Completed
            };
            (
                status,
                tr.is_error,
                Some(cap.result_json),
                cap.content_kinds,
                cap.result_bytes,
                None,
            )
        }
        Err(e) => (
            classify_error_status(e),
            true,
            None,
            Vec::new(),
            0,
            Some(e.to_string()),
        ),
    };

    Some(CreateMcpToolCall {
        server_id: Some(server_id),
        server_name: ctx.server_name.clone(),
        is_built_in: ctx.is_built_in,
        user_id,
        conversation_id: ctx.conversation_id,
        branch_id: ctx.branch_id,
        message_id: ctx.message_id,
        tool_use_id: ctx.tool_use_id.clone(),
        tool_name: tool_name.to_string(),
        arguments_json: cap_arguments(arguments),
        source: ctx.source,
        status,
        is_error,
        result_json,
        content_kinds,
        result_bytes,
        error_message,
        started_at,
        finished_at: Some(started_at + time::Duration::milliseconds(elapsed_ms)),
        duration_ms: Some(elapsed_ms),
        workflow_run_id: ctx.workflow_run_id,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::mcp::client::{ToolContent, ToolResult};
    use crate::modules::mcp::tool_calls::models::{McpToolCallSource, McpToolCallStatus};

    fn ctx_with_owner() -> McpCallContext {
        McpCallContext {
            user_id: Some(Uuid::nil()),
            server_name: "mock".into(),
            ..Default::default()
        }
    }

    fn block(v: Value) -> ToolContent {
        ToolContent { content: v }
    }

    fn ok_result(content: Vec<ToolContent>, is_error: bool) -> Result<ToolResult, AppError> {
        Ok(ToolResult {
            content,
            is_error,
            structured_content: None,
        })
    }

    #[test]
    fn cap_arguments_passes_small_and_truncates_large() {
        let small = json!({ "q": "hello" });
        assert_eq!(cap_arguments(&small), small);

        let big = json!({ "q": "x".repeat(MAX_ARGS_BYTES + 100) });
        let capped = cap_arguments(&big);
        assert_eq!(capped["_truncated"], json!(true));
        assert!(capped["_bytes"].as_u64().unwrap() as usize > MAX_ARGS_BYTES);
    }

    #[test]
    fn capture_scrubs_ziee_host_paths_keeps_workflow_handles() {
        // A code_sandbox-style resource_link result captured BEFORE the chat scrub: the raw
        // `ziee://<host_path>` must not be persisted into the queryable history.
        let result = ToolResult {
            content: vec![block(json!({
                "type": "resource_link",
                "uri": "ziee:///Users/x/.ziee/sandboxes/c/out.csv",
                "name": "out.csv",
            }))],
            is_error: false,
            structured_content: Some(json!({
                "uri": "ziee:///Users/x/.ziee/sandboxes/c/out.csv",
                "wf": "ziee://workflow-runs/r1/outputs/x",
            })),
        };
        let cap = capture_result(&result);
        let serialized = serde_json::to_string(&cap.result_json).unwrap();
        // No absolute host-path ziee:// survives (content + structured_content).
        assert!(!serialized.contains("ziee:///"), "host path leaked: {serialized}");
        // workflow_mcp logical handle is preserved (not a host path).
        assert!(serialized.contains("ziee://workflow-runs/"), "workflow handle dropped");
        assert_eq!(cap.result_json["content"][0]["uri"], json!(""));
    }

    #[test]
    fn capture_strips_base64_and_records_kinds_and_presize() {
        let result = ToolResult {
            content: vec![
                block(json!({ "type": "text", "text": "hi" })),
                block(json!({ "type": "image", "mimeType": "image/png", "data": "AAAABBBB" })),
                block(json!({ "type": "text", "text": "again" })),
            ],
            is_error: false,
            structured_content: None,
        };
        let cap = capture_result(&result);

        // distinct kinds, first-seen order, no dupes
        assert_eq!(cap.content_kinds, vec!["text".to_string(), "image".to_string()]);
        // pre-strip size counts the original base64 bytes
        assert!(cap.result_bytes > 0);
        // base64 `data` stripped to a reference
        let img = &cap.result_json["content"][1];
        assert_eq!(img["data"]["_stripped"], json!(true));
        assert_eq!(img["data"]["_bytes"], json!(8));
        // text preserved
        assert_eq!(cap.result_json["content"][0]["text"], json!("hi"));
    }

    #[test]
    fn build_record_maps_status_from_outcome() {
        let ctx = ctx_with_owner();
        let started = time::OffsetDateTime::UNIX_EPOCH;
        let args = json!({});

        let completed = build_record(
            Uuid::nil(),
            &ctx,
            "t",
            &args,
            &ok_result(vec![block(json!({"type":"text","text":"ok"}))], false),
            started,
            5,
        )
        .unwrap();
        assert_eq!(completed.status, McpToolCallStatus::Completed);
        assert!(!completed.is_error);
        assert_eq!(completed.duration_ms, Some(5));

        let tool_error = build_record(
            Uuid::nil(),
            &ctx,
            "t",
            &args,
            &ok_result(vec![], true),
            started,
            1,
        )
        .unwrap();
        assert_eq!(tool_error.status, McpToolCallStatus::Failed);
        assert!(tool_error.is_error);

        let timed_out: Result<ToolResult, AppError> =
            Err(AppError::internal_error("request timed out"));
        let rec = build_record(Uuid::nil(), &ctx, "t", &args, &timed_out, started, 1).unwrap();
        assert_eq!(rec.status, McpToolCallStatus::Timeout);
        assert!(rec.error_message.is_some());

        let boom: Result<ToolResult, AppError> = Err(AppError::internal_error("boom"));
        let rec = build_record(Uuid::nil(), &ctx, "t", &args, &boom, started, 1).unwrap();
        assert_eq!(rec.status, McpToolCallStatus::Failed);
    }

    #[test]
    fn build_record_carries_workflow_run_id() {
        // E4: a workflow-dispatched tool call records its run link.
        let mut ctx = ctx_with_owner();
        let run = Uuid::from_u128(0x1234_5678);
        ctx.workflow_run_id = Some(run);
        let rec = build_record(
            Uuid::nil(),
            &ctx,
            "t",
            &json!({}),
            &ok_result(vec![block(json!({"type":"text","text":"ok"}))], false),
            time::OffsetDateTime::UNIX_EPOCH,
            1,
        )
        .unwrap();
        assert_eq!(rec.workflow_run_id, Some(run));
    }

    #[test]
    fn build_record_carries_is_built_in_for_builtin_servers() {
        // A built-in MCP server's tool call (e.g. files_mcp's read_file) must be
        // recorded into mcp_tool_calls with is_built_in=true so the history
        // surface can distinguish built-ins from user/external servers.
        let mut ctx = ctx_with_owner();
        ctx.is_built_in = true;
        ctx.server_name = "files".into();
        let rec = build_record(
            Uuid::nil(),
            &ctx,
            "read_file",
            &json!({ "file_id": "abc" }),
            &ok_result(vec![block(json!({"type":"text","text":"contents"}))], false),
            time::OffsetDateTime::UNIX_EPOCH,
            1,
        )
        .expect("an owner-stamped session must record");
        assert!(rec.is_built_in, "built-in server tool call must set is_built_in=true");
        assert_eq!(rec.tool_name, "read_file");
        assert_eq!(rec.server_name, "files");

        // Contrast: a non-built-in (user/external) context records is_built_in=false.
        let ext = ctx_with_owner(); // is_built_in defaults to false
        let rec2 = build_record(
            Uuid::nil(),
            &ext,
            "do_thing",
            &json!({}),
            &ok_result(vec![], false),
            time::OffsetDateTime::UNIX_EPOCH,
            1,
        )
        .unwrap();
        assert!(!rec2.is_built_in, "external server tool call must set is_built_in=false");
    }

    #[test]
    fn build_record_skips_unstamped_session() {
        let ctx = McpCallContext::default(); // user_id = None
        let started = time::OffsetDateTime::UNIX_EPOCH;
        let rec = build_record(
            Uuid::nil(),
            &ctx,
            "t",
            &json!({}),
            &ok_result(vec![], false),
            started,
            1,
        );
        assert!(rec.is_none());
    }

    #[test]
    fn sanitize_redacts_secrets_and_preserves_short_nonbinary_data() {
        let v = json!({
            "authorization": "Bearer abc123",
            "nested": { "api_key": "k", "ok": 1 },
            // A non-binary block: short "data" must be preserved (no `type` here).
            "structuredContent": { "data": "OK" },
        });
        let out = sanitize_value(v);
        assert_eq!(out["authorization"], json!("[redacted]"));
        assert_eq!(out["nested"]["api_key"], json!("[redacted]"));
        assert_eq!(out["nested"]["ok"], json!(1), "non-secret values pass through");
        assert_eq!(
            out["structuredContent"]["data"],
            json!("OK"),
            "data outside a binary content block is NOT stripped"
        );
    }

    #[test]
    fn sanitize_strips_nested_embedded_resource_blob() {
        // MCP embedded resource: bytes live at `resource.blob`, one level below
        // the block's `type` — must still be stripped.
        let v = json!({
            "type": "resource",
            "resource": {
                "uri": "file:///x",
                "mimeType": "application/octet-stream",
                "blob": "QUJDREVGR0g=",
                "text": "kept"
            }
        });
        let out = sanitize_value(v);
        assert_eq!(out["resource"]["blob"]["_stripped"], json!(true));
        assert_eq!(out["resource"]["uri"], json!("file:///x"), "uri preserved");
        assert_eq!(out["resource"]["text"], json!("kept"), "text preserved");
    }

    #[test]
    fn capture_caps_oversized_result() {
        let big = "x".repeat(MAX_RESULT_BYTES + 100);
        let result = ToolResult {
            content: vec![block(json!({ "type": "text", "text": big }))],
            is_error: false,
            structured_content: None,
        };
        let cap = capture_result(&result);
        assert_eq!(cap.result_json["_truncated"], json!(true));
        assert!(cap.result_bytes as usize > MAX_RESULT_BYTES, "pre-strip size recorded");
    }

    #[test]
    fn enum_as_str_round_trips() {
        assert_eq!(McpToolCallStatus::Completed.as_str(), "completed");
        assert_eq!(McpToolCallStatus::Timeout.as_str(), "timeout");
        assert_eq!(McpToolCallSource::Rest.as_str(), "rest");
        assert_eq!(McpToolCallSource::default().as_str(), "chat");
    }
}
