//! HTTP handlers: the JSON-RPC MCP endpoint + the admin settings REST surface.

use aide::transform::TransformOperation;
use axum::{
    Json, debug_handler,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::common::{ApiResult, AppError};
use crate::core::Repos;
use crate::modules::code_sandbox::types::{
    ConversationIdHeader, JsonRpcError, JsonRpcRequest, JsonRpcResponse,
};
use crate::modules::permissions::{RequirePermissions, with_permission};

use super::models::{OfficeBridgeSettings, UpdateOfficeBridgeSettingsRequest};
use super::permissions::{OfficeBridgeAdminRead, OfficeBridgeManage, OfficeBridgeUse};
use super::platform::{self, DocOp, OfficeApp, OfficePlatform};

// ─────────────────────────── JSON-RPC MCP endpoint ───────────────────────────

#[debug_handler]
pub async fn jsonrpc_handler(
    // Gated on `office_bridge::use`; the JWT is validated by the extractor.
    // Conversation id is accepted but unused in this increment.
    _auth: RequirePermissions<(OfficeBridgeUse,)>,
    ConversationIdHeader(_conversation_id): ConversationIdHeader,
    body: axum::body::Bytes,
) -> Response {
    let raw: Value = match serde_json::from_slice(&body) {
        Ok(v) => v,
        Err(e) => {
            return error_response(
                None,
                StatusCode::BAD_REQUEST,
                JsonRpcError::parse_error(e.to_string()),
            );
        }
    };
    let req: JsonRpcRequest = match serde_json::from_value(raw) {
        Ok(r) => r,
        Err(e) => {
            return error_response(
                None,
                StatusCode::BAD_REQUEST,
                JsonRpcError::invalid_request(e.to_string()),
            );
        }
    };

    // Notifications carry no `id`, expect no response.
    if req.id.is_none() {
        return StatusCode::ACCEPTED.into_response();
    }
    let id = req.id.clone();

    match req.method.as_str() {
        "initialize" => ok_response(
            id,
            json!({
                "protocolVersion": "2025-11-25",
                "capabilities": { "tools": {} },
                "serverInfo": { "name": "office_bridge", "version": env!("CARGO_PKG_VERSION") },
            }),
        ),
        "tools/list" => ok_response(id, super::tools::tool_list()),
        "ping" => ok_response(id, json!({})),
        "tools/call" => match dispatch_tool_call(&req.params).await {
            Ok(value) => ok_response(id, value),
            Err(e) => error_response(id, e.0, e.1),
        },
        _ => error_response(
            id,
            StatusCode::OK,
            JsonRpcError::method_not_found(&req.method),
        ),
    }
}

fn ok_response(id: Option<Value>, result: Value) -> Response {
    (
        StatusCode::OK,
        Json(JsonRpcResponse {
            jsonrpc: "2.0",
            id,
            result: Some(result),
            error: None,
        }),
    )
        .into_response()
}

fn error_response(id: Option<Value>, http: StatusCode, err: JsonRpcError) -> Response {
    (
        http,
        Json(JsonRpcResponse {
            jsonrpc: "2.0",
            id,
            result: None,
            error: Some(err),
        }),
    )
        .into_response()
}

// ─────────────────────────── `office` tool dispatch ──────────────────────────

#[derive(Debug, Deserialize)]
struct ToolCallParams {
    name: String,
    #[serde(default)]
    arguments: Value,
}

/// Parse the `tools/call` params and dispatch against the live platform
/// (`platform::active()` in production — the COM daemon on Windows). Errors are
/// mapped to the client-class JSON-RPC error the LLM should see, never a raw
/// server crash.
async fn dispatch_tool_call(params: &Value) -> Result<Value, (StatusCode, JsonRpcError)> {
    let call: ToolCallParams = serde_json::from_value(params.clone()).map_err(|e| {
        (
            StatusCode::OK,
            JsonRpcError::invalid_params(format!("tools/call params: {e}")),
        )
    })?;

    match dispatch_tool(platform::active(), &call.name, &call.arguments).await {
        Ok(value) => Ok(value),
        Err(e) => Err((StatusCode::OK, JsonRpcError::from_app_error(&e))),
    }
}

/// The distinct error code for a tool that needs an open Office.js task pane —
/// the daemon→pane bridge RPC is a later item, so these tools answer honestly
/// instead of pretending to act. 422 keeps it client-class (→ invalid_params).
const OFFICE_PANE_REQUIRED: &str = "OFFICE_PANE_REQUIRED";
/// The distinct error code for an operation that the target host application
/// does not support (the proven capability matrix: PowerPoint has no comments /
/// tracked changes).
const OFFICE_UNSUPPORTED_ON_HOST: &str = "OFFICE_UNSUPPORTED_ON_HOST";

fn pane_required_err(op: &str) -> AppError {
    AppError::new(
        StatusCode::UNPROCESSABLE_ENTITY,
        OFFICE_PANE_REQUIRED,
        format!(
            "`{op}` requires the document's Office task pane to be open; the \
             daemon-to-pane bridge is not yet wired, so this operation is not \
             available right now. `list_open_documents` and `edit_document` \
             (append_paragraph) work today."
        ),
    )
}

fn unsupported_on_ppt_err(op: &str) -> AppError {
    AppError::new(
        StatusCode::UNPROCESSABLE_ENTITY,
        OFFICE_UNSUPPORTED_ON_HOST,
        format!("`{op}` is not supported on PowerPoint documents."),
    )
}

/// Pure, injectable `office` tool dispatcher. Takes the platform as a trait
/// object so tests can pass a `MockOfficePlatform`; production passes
/// `platform::active()`. Returns the MCP `tools/call` result body (`content` +
/// `structuredContent`, mirroring web_search) on success.
///
/// Capability model for this increment:
/// - `list_open_documents` + `edit_document`(append_paragraph) route to the
///   native daemon and work now.
/// - `read_document` / `get_selection` / `add_comment` / `set_track_changes` /
///   `get_tracked_changes` are pane-mediated (Office.js) and return a typed
///   "requires task pane" capability error until that RPC lands.
/// - `add_comment` / `set_track_changes` targeting a PowerPoint document return
///   the distinct "unsupported on PowerPoint" error where the host is known.
pub async fn dispatch_tool(
    platform: &dyn OfficePlatform,
    name: &str,
    args: &Value,
) -> Result<Value, AppError> {
    match name {
        "list_open_documents" => {
            let docs = platform.list_open_documents().await?;
            let mut text = format!("{} open Office document(s).\n", docs.len());
            for d in &docs {
                text.push_str(&format!(
                    "- {} [{:?}] {} ({})\n",
                    d.name,
                    d.app,
                    d.full_name,
                    if d.saved { "saved" } else { "unsaved" }
                ));
            }
            let structured = json!({ "documents": docs });
            Ok(tool_result(text, structured))
        }

        "edit_document" => {
            let a: EditDocumentArgs = parse_args(args)?;
            match a.op.as_str() {
                "append_paragraph" => {
                    let text = a.text.unwrap_or_default();
                    let res = platform
                        .act_on_document(&a.doc_full_name, &DocOp::AppendParagraph { text })
                        .await?;
                    let msg = if res.ok {
                        format!("Appended a paragraph to {}.", a.doc_full_name)
                    } else {
                        format!("Edit to {} did not apply.", a.doc_full_name)
                    };
                    let structured = json!({ "ok": res.ok, "read_back": res.read_back });
                    Ok(tool_result(msg, structured))
                }
                other => Err(AppError::bad_request(
                    "OFFICE_UNKNOWN_OP",
                    format!("unknown edit_document op: `{other}` (supported: append_paragraph)"),
                )),
            }
        }

        // Pane-mediated: no host-capability nuance — always "needs a task pane".
        "read_document" | "get_selection" | "get_tracked_changes" => Err(pane_required_err(name)),

        // Pane-mediated AND host-gated: surface the PowerPoint-unsupported error
        // distinctly where the target document's host is known.
        "add_comment" | "set_track_changes" => {
            let doc_full_name = args
                .get("doc_full_name")
                .and_then(|v| v.as_str())
                .ok_or_else(|| {
                    AppError::bad_request(
                        "INVALID_ARGS",
                        "missing required `doc_full_name` argument",
                    )
                })?;
            if doc_host(platform, doc_full_name).await == Some(OfficeApp::PowerPoint) {
                return Err(unsupported_on_ppt_err(name));
            }
            Err(pane_required_err(name))
        }

        other => Err(AppError::bad_request(
            "UNKNOWN_TOOL",
            format!("unknown office tool: `{other}`"),
        )),
    }
}

/// Wrap a `(readable text, structured payload)` pair as the MCP `tools/call`
/// result body — `content` is what the LLM reads, `structuredContent` is the
/// typed copy the UI renders / the model recalls (mirrors web_search).
fn tool_result(text: impl Into<String>, structured: Value) -> Value {
    json!({
        "content": [{ "type": "text", "text": text.into() }],
        "structuredContent": structured,
    })
}

fn parse_args<T: serde::de::DeserializeOwned>(args: &Value) -> Result<T, AppError> {
    serde_json::from_value(args.clone())
        .map_err(|e| AppError::bad_request("INVALID_ARGS", e.to_string()))
}

/// Resolve the host application of an open document by its full name, or `None`
/// if it is not currently enumerated (so callers fall back to the generic
/// pane-required error rather than a spurious capability claim).
async fn doc_host(platform: &dyn OfficePlatform, doc_full_name: &str) -> Option<OfficeApp> {
    platform
        .list_open_documents()
        .await
        .ok()?
        .into_iter()
        .find(|d| d.full_name == doc_full_name)
        .map(|d| d.app)
}

#[derive(Debug, Deserialize)]
struct EditDocumentArgs {
    doc_full_name: String,
    op: String,
    #[serde(default)]
    text: Option<String>,
}

// ─────────────────────────── Admin REST: settings ───────────────────────────

#[debug_handler]
pub async fn get_settings(
    _auth: RequirePermissions<(OfficeBridgeAdminRead,)>,
) -> ApiResult<Json<OfficeBridgeSettings>> {
    let row = Repos.office_bridge.get_settings().await?;
    Ok((StatusCode::OK, Json(row)))
}

pub fn get_settings_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(OfficeBridgeAdminRead,)>(op)
        .id("OfficeBridge.getSettings")
        .tag("OfficeBridge")
        .summary("Read office-bridge settings")
        .response::<200, Json<OfficeBridgeSettings>>()
}

#[debug_handler]
pub async fn update_settings(
    _auth: RequirePermissions<(OfficeBridgeManage,)>,
    Json(body): Json<UpdateOfficeBridgeSettingsRequest>,
) -> ApiResult<Json<OfficeBridgeSettings>> {
    if let Some(port) = body.port
        && !(1..=65535).contains(&port)
    {
        return Err(
            AppError::bad_request("VALIDATION_ERROR", "port out of range (1..=65535)").into(),
        );
    }

    let row = Repos
        .office_bridge
        .update_settings(body.enabled, body.port)
        .await?;
    Ok((StatusCode::OK, Json(row)))
}

pub fn update_settings_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(OfficeBridgeManage,)>(op)
        .id("OfficeBridge.updateSettings")
        .tag("OfficeBridge")
        .summary("Update office-bridge settings (enable, port)")
        .response::<200, Json<OfficeBridgeSettings>>()
}

// ─────────────────────────────────── Tests ──────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::office_bridge::platform::{MockOfficePlatform, OpenDoc};

    /// A mock seeded with a couple of open docs INCLUDING a PowerPoint one, so
    /// the capability-matrix branch (`add_comment`/`set_track_changes` on PPT)
    /// is exercisable. Mirrors the `MockOfficePlatform::new()` shape but adds
    /// the PowerPoint deck the default seed lacks.
    fn seeded_mock() -> MockOfficePlatform {
        MockOfficePlatform::with_docs(vec![
            OpenDoc {
                app: OfficeApp::Word,
                name: "Report.docx".to_string(),
                full_name: r"C:\Users\test\Report.docx".to_string(),
                path: Some(r"C:\Users\test".to_string()),
                saved: true,
                active: true,
                attach_method: "mock".to_string(),
            },
            OpenDoc {
                app: OfficeApp::PowerPoint,
                name: "Deck.pptx".to_string(),
                full_name: r"C:\Users\test\Deck.pptx".to_string(),
                path: Some(r"C:\Users\test".to_string()),
                saved: true,
                active: false,
                attach_method: "mock".to_string(),
            },
        ])
    }

    /// TEST-12 (a) — `list_open_documents` dispatches to the platform and returns
    /// the seeded docs in `structuredContent`.
    #[tokio::test]
    async fn test12_list_open_documents_returns_seeded_docs() {
        let mock = seeded_mock();
        let out = dispatch_tool(&mock, "list_open_documents", &json!({}))
            .await
            .expect("list_open_documents succeeds");
        let docs = out["structuredContent"]["documents"]
            .as_array()
            .expect("documents array");
        assert_eq!(docs.len(), 2);
        assert_eq!(docs[0]["name"], "Report.docx");
        assert_eq!(docs[1]["app"], "power_point");
        // The readable text channel is present for the LLM.
        assert!(out["content"][0]["text"].is_string());
    }

    /// TEST-12 (b) — `edit_document`(append_paragraph) routes to the daemon and
    /// returns `ok` + a `read_back` of the appended text.
    #[tokio::test]
    async fn test12_edit_document_append_returns_ok_and_read_back() {
        let mock = seeded_mock();
        let out = dispatch_tool(
            &mock,
            "edit_document",
            &json!({
                "doc_full_name": r"C:\Users\test\Report.docx",
                "op": "append_paragraph",
                "text": "hello world",
            }),
        )
        .await
        .expect("edit_document succeeds");
        assert_eq!(out["structuredContent"]["ok"], true);
        assert_eq!(out["structuredContent"]["read_back"], "hello world");
    }

    /// TEST-12 (c) — `add_comment` on a PowerPoint doc returns the distinct
    /// "unsupported on PowerPoint" capability error (not a crash), where the host
    /// is known from enumeration.
    #[tokio::test]
    async fn test12_add_comment_on_powerpoint_returns_capability_error() {
        let mock = seeded_mock();
        let err = dispatch_tool(
            &mock,
            "add_comment",
            &json!({
                "doc_full_name": r"C:\Users\test\Deck.pptx",
                "anchor_text": "Agenda",
                "text": "revise this slide",
            }),
        )
        .await
        .expect_err("add_comment on PPT is a capability error");
        assert_eq!(err.error_code(), OFFICE_UNSUPPORTED_ON_HOST);
        assert_eq!(err.status_code(), 422);
    }

    /// TEST-12 (c cont.) — `set_track_changes` on PowerPoint likewise.
    #[tokio::test]
    async fn test12_set_track_changes_on_powerpoint_returns_capability_error() {
        let mock = seeded_mock();
        let err = dispatch_tool(
            &mock,
            "set_track_changes",
            &json!({ "doc_full_name": r"C:\Users\test\Deck.pptx", "enabled": true }),
        )
        .await
        .expect_err("set_track_changes on PPT is a capability error");
        assert_eq!(err.error_code(), OFFICE_UNSUPPORTED_ON_HOST);
    }

    /// TEST-12 (d) — a pane-mediated method (`get_selection`) returns the typed
    /// "requires task pane" error, honestly signalling the not-yet-wired
    /// capability rather than panicking or 500-ing.
    #[tokio::test]
    async fn test12_pane_mediated_method_returns_pane_required_error() {
        let mock = seeded_mock();
        for tool in ["get_selection", "read_document", "get_tracked_changes"] {
            let err = dispatch_tool(
                &mock,
                tool,
                &json!({ "doc_full_name": r"C:\Users\test\Report.docx" }),
            )
            .await
            .err()
            .unwrap_or_else(|| panic!("`{tool}` should error"));
            assert_eq!(err.error_code(), OFFICE_PANE_REQUIRED, "for tool {tool}");
            assert_eq!(err.status_code(), 422, "for tool {tool}");
        }
    }

    /// `add_comment` on a Word doc (host known, but not PowerPoint) falls through
    /// to the generic pane-required error, NOT the PowerPoint capability error.
    #[tokio::test]
    async fn test12_add_comment_on_word_returns_pane_required_error() {
        let mock = seeded_mock();
        let err = dispatch_tool(
            &mock,
            "add_comment",
            &json!({
                "doc_full_name": r"C:\Users\test\Report.docx",
                "anchor_text": "Intro",
                "text": "expand this",
            }),
        )
        .await
        .expect_err("add_comment on Word is pane-required for now");
        assert_eq!(err.error_code(), OFFICE_PANE_REQUIRED);
    }

    /// An unknown tool name is a client-class error, never a panic.
    #[tokio::test]
    async fn test12_unknown_tool_is_client_error() {
        let mock = seeded_mock();
        let err = dispatch_tool(&mock, "nonexistent_tool", &json!({}))
            .await
            .expect_err("unknown tool errors");
        assert_eq!(err.error_code(), "UNKNOWN_TOOL");
    }
}
