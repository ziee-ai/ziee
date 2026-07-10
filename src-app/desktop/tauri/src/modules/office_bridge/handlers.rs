//! HTTP handlers: the JSON-RPC MCP endpoint + the admin settings REST surface.

use std::path::{Path, PathBuf};

use aide::transform::TransformOperation;
use axum::{
    Json, debug_handler,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Deserialize;
use serde_json::{Value, json};

use ziee::{ApiResult, AppError};
use ziee::code_sandbox::{
    ConversationIdHeader, JsonRpcError, JsonRpcRequest, JsonRpcResponse,
};
use ziee::permissions::{RequirePermissions, with_permission};

use super::bridge::broker;
use super::models::{ConnectReadiness, OfficeBridgeSettings, UpdateOfficeBridgeSettingsRequest};
use super::permissions::{OfficeBridgeAdminRead, OfficeBridgeManage, OfficeBridgeUse};
use super::platform::{self, OfficeApp, OfficePlatform, OpenDoc};

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
    // Defense-in-depth (mirrors the chat-attach path's `settings.enabled` gate):
    // re-check the runtime admin toggle before executing so an admin who has
    // runtime-disabled the module gets a typed "disabled" error instead of the
    // tools running, even though `office_bridge::use` is still granted. A cheap
    // DB read like the other settings reads.
    let settings = crate::modules::office_bridge::OfficeBridgeRepository::new(ziee::Repos.pool().clone())
        .get_settings()
        .await
        .map_err(|e| (StatusCode::OK, JsonRpcError::from_app_error(&e)))?;
    if !settings.enabled {
        return Err((
            StatusCode::OK,
            JsonRpcError::from_app_error(&office_bridge_disabled_err()),
        ));
    }

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

/// The typed error `tools/call` returns when the module is runtime-disabled by
/// an admin (`office_bridge_settings.enabled == false`). 403 keeps it distinct
/// from a client's malformed request.
const OFFICE_BRIDGE_DISABLED: &str = "OFFICE_BRIDGE_DISABLED";
fn office_bridge_disabled_err() -> AppError {
    AppError::new(
        StatusCode::FORBIDDEN,
        OFFICE_BRIDGE_DISABLED,
        "the office bridge is disabled by the administrator; office tools are not available",
    )
}

/// The distinct error code for an operation that the target host application
/// does not support (the proven capability matrix: PowerPoint has no comments /
/// tracked changes).
const OFFICE_UNSUPPORTED_ON_HOST: &str = "OFFICE_UNSUPPORTED_ON_HOST";

/// Extract the required `doc_full_name` argument (shared by every pane tool).
fn require_doc_full_name(args: &Value) -> Result<String, AppError> {
    args.get("doc_full_name")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| {
            AppError::bad_request("INVALID_ARGS", "missing required `doc_full_name` argument")
        })
}

/// Wrap a pane's JSON-RPC result into the MCP `tools/call` result shape. The pane
/// returns a per-op object (e.g. `{text}` for read/selection, `{ok}` for a mutation,
/// `{changes}` for tracked changes); we surface its `text` field (when present) as the
/// readable channel and pass the whole object through as `structuredContent`.
fn pane_tool_result(tool: &str, doc: &str, result: Value) -> Value {
    let text = result
        .get("text")
        .and_then(|t| t.as_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| format!("`{tool}` on {doc} completed."));
    tool_result(text, result)
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
/// Capability model:
/// - `list_open_documents` routes to the native daemon (`platform::active()` —
///   osascript/COM); it needs no task pane.
/// - `read_document` / `get_selection` / `run_office_js` / `add_comment` /
///   `set_track_changes` / `get_tracked_changes` are pane-mediated (Office.js) and
///   route to the connected task pane via `broker::call_pane`, which maps no-pane /
///   timeout / pane-error to typed errors (`OFFICE_PANE_NOT_CONNECTED` /
///   `OFFICE_PANE_TIMEOUT` / `OFFICE_PANE_ERROR` / `OFFICE_UNSUPPORTED_ON_HOST`).
/// - `read_document` / `get_selection` / `run_office_js` are host-agnostic (Word +
///   Excel + PowerPoint) — no capability pre-gate; the pane picks the host runtime.
/// - `add_comment` / `set_track_changes` / `get_tracked_changes` (Word-only) first
///   fast-gate a PowerPoint target natively (`OFFICE_UNSUPPORTED_ON_HOST`) before any
///   round-trip; a non-Word non-PPT host is caught by the pane's `-32002` → same code.
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

        // Pane-mediated (Office.js over the WSS bridge): route to the connected task
        // pane via the broker and await the correlated reply. `read_document` +
        // `get_selection` are host-agnostic (Word + Excel + PowerPoint), so no
        // capability pre-gate (`run_office_js`, also host-agnostic, is the next arm).
        "read_document" | "get_selection" => {
            let doc_full_name = require_doc_full_name(args)?;
            let result = broker::call_pane(&doc_full_name, name, args.clone()).await?;
            Ok(pane_tool_result(name, &doc_full_name, result))
        }

        // Open-ended Office.js execution: the model supplies a `script` body the pane
        // runs inside the host's `{Word,Excel,PowerPoint}.run`. Validate a non-empty
        // script before the round-trip (typed `INVALID_ARGS`), then route to the pane;
        // the pane returns the script's value or a structured error (`OFFICE_PANE_ERROR`).
        // Per-call user approval is enforced upstream (office_bridge is absent from the
        // approval-bypass set), so arbitrary-code risk is user-consented per call.
        //
        // Routing/targeting: same `broker::call_pane` path (and the same pane-side
        // `sameDoc` mis-routing guard) as the other pane tools — a request whose
        // `doc_full_name` doesn't match a connected pane's own document is rejected
        // pane-side; the sole-pane fallback for an unsaved (keyless) doc is the
        // deliberate pane-rpc routing model, unchanged here. Runaway scripts: a
        // synchronously non-terminating script wedges the pane's own JS thread (an
        // inherent limit of arbitrary code in a single-threaded WebView), but the
        // broker's `CALL_TIMEOUT` bounds THIS call — the caller gets `OFFICE_PANE_TIMEOUT`.
        "run_office_js" => {
            let doc_full_name = require_doc_full_name(args)?;
            let script_ok = args
                .get("script")
                .and_then(|v| v.as_str())
                .is_some_and(|s| !s.trim().is_empty());
            if !script_ok {
                return Err(AppError::bad_request(
                    "INVALID_ARGS",
                    "`run_office_js` requires a non-empty `script` argument",
                ));
            }
            let result = broker::call_pane(&doc_full_name, name, args.clone()).await?;
            Ok(pane_tool_result(name, &doc_full_name, result))
        }

        // Pane-mediated AND Word-only: surface the PowerPoint-unsupported error
        // distinctly (via the fast native `doc_host` lookup) BEFORE any round-trip,
        // then route to the pane. (`get_tracked_changes` is grouped here — Word-only —
        // so PPT gets the same fast `OFFICE_UNSUPPORTED_ON_HOST` as the mutations; a
        // non-Word non-PPT host is caught by the pane's `-32002` → same code.)
        "add_comment" | "set_track_changes" | "get_tracked_changes" => {
            let doc_full_name = require_doc_full_name(args)?;
            if doc_host(platform, &doc_full_name).await == Some(OfficeApp::PowerPoint) {
                return Err(unsupported_on_ppt_err(name));
            }
            let result = broker::call_pane(&doc_full_name, name, args.clone()).await?;
            Ok(pane_tool_result(name, &doc_full_name, result))
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

// ─────────────────────────── Admin REST: settings ───────────────────────────

#[debug_handler]
pub async fn get_settings(
    _auth: RequirePermissions<(OfficeBridgeAdminRead,)>,
) -> ApiResult<Json<OfficeBridgeSettings>> {
    let row = crate::modules::office_bridge::OfficeBridgeRepository::new(ziee::Repos.pool().clone()).get_settings().await?;
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

    let row = crate::modules::office_bridge::OfficeBridgeRepository::new(ziee::Repos.pool().clone())
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

// ─────────────────────────── Admin REST: [Connect] ──────────────────────────

/// Run the one-shot `[Connect]` install steps against `platform` and report
/// readiness (ITEM-13). Pure + injectable (takes the platform as a trait object)
/// so tests can drive it with a `MockOfficePlatform` without a real Office
/// install; production passes `platform::active()`.
///
/// Every platform call is **best-effort**: a failure sets that step's boolean to
/// `false` and appends a note to `message`, rather than propagating an error and
/// 500-ing the request. This lets the admin see a partial-success report (e.g.
/// "cert trusted, but Office is elevated") instead of an opaque failure.
///
/// Steps:
/// - `probe()` → `office_present`.
/// - `install_cert_trust(ca_der)` (one UAC on Windows) → `cert_trusted`.
/// - `register_sideload(manifest_path)` → `sideloaded`.
/// - `office_is_elevated()` → `office_elevated_warning` (true = warn: an elevated
///   Office disables the add-in platform and cannot be automated).
pub fn run_connect(
    platform: &dyn OfficePlatform,
    ca_der: &[u8],
    manifest_path: &Path,
    bridge_port: i32,
) -> ConnectReadiness {
    let mut notes: Vec<String> = Vec::new();

    // Office presence (a supported desktop with Office *absent* still connects so
    // the admin can be told to open a document).
    let office_present = platform.probe().map(|c| c.office_present).unwrap_or(false);
    if !office_present {
        notes.push(
            "No Microsoft Office installation was detected; open Word, Excel, or \
             PowerPoint before using the bridge."
                .to_string(),
        );
    }

    // Trust the bridge CA (one elevation prompt on Windows) — best-effort.
    let cert_trusted = match platform.install_cert_trust(ca_der) {
        Ok(()) => true,
        Err(e) => {
            notes.push(format!("Trusting the bridge certificate failed: {e}."));
            false
        }
    };

    // Register the add-in manifest for sideloading — best-effort.
    let sideloaded = match platform.register_sideload(manifest_path) {
        Ok(()) => true,
        Err(e) => {
            notes.push(format!("Registering the add-in for sideloading failed: {e}."));
            false
        }
    };

    // Elevated-Office warning (COM same-integrity rule — an elevated Office
    // cannot be automated from the non-elevated daemon, and the add-in platform
    // is disabled for it).
    let office_elevated_warning = platform.office_is_elevated();
    if office_elevated_warning {
        notes.push(
            "Microsoft Office is running elevated (as administrator); the add-in \
             platform is disabled for elevated Office. Restart Office without \
             administrator rights."
                .to_string(),
        );
    }

    if notes.is_empty() {
        notes.push(
            "Office bridge connected: the certificate is trusted and the add-in is \
             sideloaded. Use the ribbon button to open the task pane."
                .to_string(),
        );
    }

    ConnectReadiness {
        office_present,
        office_elevated_warning,
        cert_trusted,
        sideloaded,
        bridge_port,
        message: notes.join(" "),
    }
}

/// Materialize the embedded add-in manifest to a real file under the data dir so
/// `register_sideload` has a path to hand to Office. The embedded manifest
/// hard-codes the default bridge port (44300 — the value baked into every
/// `https://localhost:44300/...` URL); when the runtime port differs we rewrite
/// those references so the sideloaded manifest matches the live listener.
fn materialize_manifest(data_dir: &Path, port: i32) -> Result<PathBuf, AppError> {
    let bytes = super::bridge::assets::get("manifest.xml")
        .ok_or_else(|| AppError::internal_error("embedded office-bridge manifest.xml is missing"))?;
    let mut xml = String::from_utf8(bytes.to_vec())
        .map_err(|e| AppError::internal_error(format!("manifest.xml is not utf-8: {e}")))?;
    if port != 44300 {
        xml = xml.replace(":44300", &format!(":{port}"));
    }
    let dir = data_dir.join("office-bridge");
    std::fs::create_dir_all(&dir)
        .map_err(|e| AppError::internal_error(format!("create {}: {e}", dir.display())))?;
    let path = dir.join("manifest.xml");
    std::fs::write(&path, xml.as_bytes())
        .map_err(|e| AppError::internal_error(format!("write {}: {e}", path.display())))?;
    Ok(path)
}

/// `POST /api/office-bridge/connect` — the admin `[Connect]` installer flow.
///
/// Loads (or mints) the bridge CA, materializes the embedded add-in manifest to
/// a real path under the data dir (injecting the configured port), then runs
/// [`run_connect`] against the live platform and returns the readiness report.
/// Gated on `office_bridge::admin::manage` by the extractor (403 without it).
#[debug_handler]
pub async fn connect(
    _auth: RequirePermissions<(OfficeBridgeManage,)>,
) -> ApiResult<Json<ConnectReadiness>> {
    let settings = crate::modules::office_bridge::OfficeBridgeRepository::new(ziee::Repos.pool().clone()).get_settings().await?;
    let port = settings.port;

    let data_dir = ziee::get_app_data_dir();
    // Mint/load the CA to trust and materialize the manifest to sideload. These
    // are genuine prerequisites (no CA / no manifest = nothing to install), so a
    // failure here is a real 500 — distinct from the best-effort platform steps
    // inside `run_connect`.
    let minted = super::bridge::cert::load_or_mint(&data_dir)?;
    let manifest_path = materialize_manifest(&data_dir, port)?;

    // `run_connect` makes blocking platform calls (a ToolHelp process-snapshot in
    // `office_is_elevated`, elevated `certutil`, HKCU registry writes), so offload
    // it to a blocking thread rather than stalling a tokio worker. `platform::active()`
    // is `&'static`; the CA bytes + manifest path are moved in owned.
    let ca_der = minted.ca_der.clone();
    let readiness = tokio::task::spawn_blocking(move || {
        run_connect(platform::active(), &ca_der, &manifest_path, port)
    })
    .await
    .map_err(|e| AppError::internal_error(format!("office_bridge: connect task join: {e}")))?;
    Ok((StatusCode::OK, Json(readiness)))
}

pub fn connect_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(OfficeBridgeManage,)>(op)
        .id("OfficeBridge.connect")
        .tag("OfficeBridge")
        .summary("Run the [Connect] installer flow (trust cert, sideload add-in, report readiness)")
        .response::<200, Json<ConnectReadiness>>()
}

// ──────────────────── User REST: open-document list (panel) ─────────────────

/// `GET /api/office-bridge/documents` — the open-Office-document list the
/// frontend "Open Office documents" panel refetches on every
/// `sync:office_document` notify (notify-and-refetch: the SSE frame carries no
/// row data, only `{entity, action, id}`, so the client re-reads here).
///
/// Gated on `office_bridge::use` — deliberately the SAME read perm the client
/// store self-gates its refetch on (the no-403 rule): a permitted user's refetch
/// never 403s, and an unpermitted store returns early without ever calling this.
///
/// **Best-effort**: enumerating open documents can fail on a non-desktop /
/// headless host (no COM, no Office) or transiently. Rather than 500 the panel we
/// log and return an empty list, so a box without Office simply renders the
/// "No open Office documents" empty state.
#[debug_handler]
pub async fn list_documents(
    _auth: RequirePermissions<(OfficeBridgeUse,)>,
) -> ApiResult<Json<Vec<OpenDoc>>> {
    let docs = match platform::active().list_open_documents().await {
        Ok(docs) => docs,
        Err(e) => {
            tracing::warn!(
                error = %e,
                "office_bridge: list_open_documents failed; returning an empty list"
            );
            Vec::new()
        }
    };
    Ok((StatusCode::OK, Json(docs)))
}

pub fn list_documents_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(OfficeBridgeUse,)>(op)
        .id("OfficeBridge.listDocuments")
        .tag("OfficeBridge")
        .summary("List the user's currently-open Office documents")
        .response::<200, Json<Vec<OpenDoc>>>()
}

// ─────────────────────────────────── Tests ──────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::office_bridge::bridge::broker;
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

    /// TEST-4 — the removed `edit_document` tool is no longer dispatchable: it now
    /// returns the generic `UNKNOWN_TOOL` client error (proving the arm — and the
    /// native `act_on_document`/`DocOp` path it drove — are gone; the crate still
    /// compiles without them).
    #[tokio::test]
    async fn test4_edit_document_is_removed_unknown_tool() {
        let mock = seeded_mock();
        let err = dispatch_tool(
            &mock,
            "edit_document",
            &json!({
                "doc_full_name": r"C:\Users\test\Report.docx",
                "op": "append_paragraph",
                "text": "hello world",
            }),
        )
        .await
        .expect_err("edit_document is removed");
        assert_eq!(err.error_code(), "UNKNOWN_TOOL");
    }

    /// TEST-5 — `run_office_js` validates its arguments before any pane round-trip:
    /// a missing `doc_full_name`, or a missing / blank `script`, is a typed
    /// `INVALID_ARGS` (not a panic, not a pane call).
    #[tokio::test]
    async fn test5_run_office_js_invalid_args() {
        let mock = seeded_mock();
        // Missing doc_full_name.
        let err = dispatch_tool(&mock, "run_office_js", &json!({ "script": "return 1;" }))
            .await
            .expect_err("missing doc_full_name is invalid");
        assert_eq!(err.error_code(), "INVALID_ARGS");
        // Missing script entirely.
        let err = dispatch_tool(
            &mock,
            "run_office_js",
            &json!({ "doc_full_name": r"C:\Users\test\Report.docx" }),
        )
        .await
        .expect_err("missing script is invalid");
        assert_eq!(err.error_code(), "INVALID_ARGS");
        // Present but blank/whitespace-only script.
        let err = dispatch_tool(
            &mock,
            "run_office_js",
            &json!({ "doc_full_name": r"C:\Users\test\Report.docx", "script": "   " }),
        )
        .await
        .expect_err("blank script is invalid");
        assert_eq!(err.error_code(), "INVALID_ARGS");
    }

    /// TEST-6 — `run_office_js` with a valid script but NO matching connected pane
    /// surfaces the typed `OFFICE_PANE_NOT_CONNECTED` (422) via the broker, not a
    /// panic (decoy panes + a UUID-unique target defeat the sole-pane fallback).
    #[tokio::test]
    async fn test6_run_office_js_no_pane_is_not_connected() {
        let decoys = register_decoy_panes();
        let mock = seeded_mock();
        let target = format!(r"C:\Users\test\RunNoPane-{}-{}.xlsx", decoys.0, decoys.1);
        let err = dispatch_tool(
            &mock,
            "run_office_js",
            &json!({ "doc_full_name": target, "script": "return 42;" }),
        )
        .await
        .expect_err("run_office_js with no pane is not connected");
        assert_eq!(err.error_code(), broker::OFFICE_PANE_NOT_CONNECTED);
        assert_eq!(err.status_code(), 422);
        unregister_decoy_panes(decoys);
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

    /// Register two decoy panes with unique, non-matching doc keys so broker
    /// resolution is deterministic (≥2 panes + no key match → not-connected,
    /// defeating the sole-pane fallback) regardless of panes other concurrent lib
    /// tests hold in the process-global broker. Returns their ids to unregister.
    fn register_decoy_panes() -> (u64, u64) {
        let (tx1, _rx1) = tokio::sync::mpsc::unbounded_channel::<axum::extract::ws::Message>();
        let (tx2, _rx2) = tokio::sync::mpsc::unbounded_channel::<axum::extract::ws::Message>();
        // Leak the receivers for the test's lifetime so the sinks stay live.
        std::mem::forget(_rx1);
        std::mem::forget(_rx2);
        let d1 = broker::next_pane_id();
        let d2 = broker::next_pane_id();
        broker::register_pane(d1, "word".into(), format!("/decoy/a-{d1}.docx"), tx1);
        broker::register_pane(d2, "excel".into(), format!("/decoy/b-{d2}.xlsx"), tx2);
        (d1, d2)
    }

    fn unregister_decoy_panes(ids: (u64, u64)) {
        broker::unregister_pane(ids.0);
        broker::unregister_pane(ids.1);
    }

    /// TEST-10 (d) — a pane-mediated method with NO matching pane connected returns
    /// the typed `OFFICE_PANE_NOT_CONNECTED` (the broker's not-connected mapping,
    /// surfaced through `dispatch_tool`), rather than panicking or 500-ing. Uses a
    /// UUID-unique target basename + decoy panes so it can't match a real/other pane.
    #[tokio::test]
    async fn test10_pane_mediated_method_no_pane_is_not_connected() {
        let decoys = register_decoy_panes();
        let mock = seeded_mock();
        let target = format!(r"C:\Users\test\NoPane-{}-{}.docx", decoys.0, decoys.1);
        for tool in ["get_selection", "read_document", "get_tracked_changes"] {
            let err = dispatch_tool(&mock, tool, &json!({ "doc_full_name": target }))
                .await
                .err()
                .unwrap_or_else(|| panic!("`{tool}` should error"));
            assert_eq!(
                err.error_code(),
                broker::OFFICE_PANE_NOT_CONNECTED,
                "for tool {tool}"
            );
            assert_eq!(err.status_code(), 422, "for tool {tool}");
        }
        unregister_decoy_panes(decoys);
    }

    /// TEST-10 (e) — `add_comment` on a Word doc (host known, not PowerPoint) passes
    /// the native PPT pre-gate and routes to the broker; with no matching pane it
    /// surfaces `OFFICE_PANE_NOT_CONNECTED`, NOT the PowerPoint capability error.
    #[tokio::test]
    async fn test10_add_comment_on_word_with_no_pane_is_not_connected() {
        let decoys = register_decoy_panes();
        let mock = seeded_mock();
        let target = format!(r"C:\Users\test\WordNoPane-{}-{}.docx", decoys.0, decoys.1);
        let err = dispatch_tool(
            &mock,
            "add_comment",
            &json!({
                "doc_full_name": target,
                "anchor_text": "Intro",
                "text": "expand this",
            }),
        )
        .await
        .expect_err("add_comment on a Word doc with no pane connected");
        assert_eq!(err.error_code(), broker::OFFICE_PANE_NOT_CONNECTED);
        unregister_decoy_panes(decoys);
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

    // ───────────────────────── TEST-16: run_connect ─────────────────────────

    // A throwaway manifest path — `run_connect` hands it verbatim to the mock's
    // `register_sideload`, which ignores it, so the path need not exist.
    fn dummy_manifest() -> std::path::PathBuf {
        std::path::PathBuf::from(r"C:\Users\test\office-bridge\manifest.xml")
    }

    /// TEST-16 — the happy path: a mock probing as Office-present, whose
    /// cert-trust + sideload succeed and which is NOT elevated, yields a fully
    /// ready report (all booleans set the expected way, the port echoed).
    #[test]
    fn test16_run_connect_all_green() {
        let mock = MockOfficePlatform::new();
        let r = run_connect(&mock, b"ca-der-bytes", &dummy_manifest(), 44300);
        assert!(r.office_present, "office present reflected from probe()");
        assert!(r.cert_trusted, "cert trust succeeded → true");
        assert!(r.sideloaded, "sideload succeeded → true");
        assert!(!r.office_elevated_warning, "not elevated → no warning");
        assert_eq!(r.bridge_port, 44300, "port echoed");
        assert!(!r.message.is_empty());
    }

    /// TEST-16 — `office_present` reflects the probe: a mock reporting Office
    /// absent produces `office_present == false` and a note in the message.
    #[test]
    fn test16_run_connect_reflects_office_absent() {
        let mock = MockOfficePlatform::new().with_office_present(false);
        let r = run_connect(&mock, b"ca", &dummy_manifest(), 44300);
        assert!(!r.office_present);
        assert!(
            r.message.to_lowercase().contains("office"),
            "absent-office note present: {}",
            r.message
        );
    }

    /// TEST-16 — a mock reporting Office elevated sets `office_elevated_warning`.
    #[test]
    fn test16_run_connect_elevated_sets_warning() {
        let mock = MockOfficePlatform::new().with_elevated(true);
        let r = run_connect(&mock, b"ca", &dummy_manifest(), 44300);
        assert!(r.office_elevated_warning, "elevated Office → warn the user");
        assert!(r.message.to_lowercase().contains("elevated"));
    }

    /// TEST-16 — each platform step is best-effort: cert-trust + sideload
    /// failures set their booleans `false` and append to `message` WITHOUT
    /// panicking or aborting (the fn always returns a report).
    #[test]
    fn test16_run_connect_step_failures_are_best_effort() {
        let mock = MockOfficePlatform::new()
            .with_cert_ok(false)
            .with_sideload_ok(false);
        let r = run_connect(&mock, b"ca", &dummy_manifest(), 12345);
        assert!(!r.cert_trusted, "cert trust failed → false");
        assert!(!r.sideloaded, "sideload failed → false");
        // Office is still present + not elevated in this mock.
        assert!(r.office_present);
        assert!(!r.office_elevated_warning);
        assert_eq!(r.bridge_port, 12345);
        // Both failure notes surfaced in the message.
        let m = r.message.to_lowercase();
        assert!(m.contains("certificate"), "cert failure noted: {}", r.message);
        assert!(m.contains("sideload"), "sideload failure noted: {}", r.message);
    }
}
