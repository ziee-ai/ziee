//! The `run_js` executor — the entry `mcp.rs` calls when it intercepts a
//! `run_js` tool_use. Wires the embedded runtime + host bridge + approval
//! together: builds the conversation's tool set into `ziee.tools.*`, runs the
//! script under an active-execution wall-clock backstop, and assembles the final
//! `McpContentData::ToolResult` (final value + captured console + per-sub-tool
//! trace + error{line}).
//!
//! Sub-tool calls re-enter the SAME dispatcher chokepoint
//! (`get_or_create_with_context` → `execute_tool`) with `sse_tx = None`, so
//! intermediate calls emit no tool cards, their results stay in the script, and
//! `mcp_tool_calls` recording is automatic (`source = script`). Gated sub-tools
//! suspend the script in-process for approval (see `approval`).

use std::collections::HashSet;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use uuid::Uuid;

use crate::modules::mcp::chat_extension::ApprovalMode;
use crate::modules::mcp::chat_extension::content::McpContentData;
use crate::modules::mcp::chat_extension::helpers::execute_tool;
use crate::modules::mcp::client::manager::McpSessionManager;
use crate::modules::mcp::tool_calls::models::McpToolCallSource;

use super::approval::{self, ApprovalCtx, ApprovalOutcome, GateDecision, SseTx};
use super::host_bridge::{self, CallBudget, DispatchFn, RawTool, ToolBinding};
use super::limits::JsCaps;
use super::runtime;

/// Everything the executor needs for one `run_js` invocation. Assembled by the
/// mcp.rs intercept from the same context it uses for the normal tool loop.
pub struct JsToolRun {
    pub session_manager: Arc<McpSessionManager>,
    pub user_id: Uuid,
    pub conversation_id: Uuid,
    pub branch_id: Uuid,
    pub message_id: Option<Uuid>,
    /// The run_js call's own tool_use id (the result is paired to it).
    pub tool_use_id: String,
    /// The conversation's accessible tool set (same list the model sees).
    pub tools: Vec<RawTool>,
    pub approval_mode: ApprovalMode,
    /// Auto-approved (server_id, tool_name) pairs (flattened allowlist).
    pub auto_approved: HashSet<(Uuid, String)>,
    pub sse_tx: SseTx,
    pub caps: JsCaps,
}

/// Shared per-run dispatch state behind the injected host functions.
struct Dispatcher {
    session_manager: Arc<McpSessionManager>,
    user_id: Uuid,
    conversation_id: Uuid,
    branch_id: Uuid,
    message_id: Option<Uuid>,
    bindings: Vec<ToolBinding>,
    approval_mode: ApprovalMode,
    auto_approved: HashSet<(Uuid, String)>,
    budget: CallBudget,
    approval_ctx: ApprovalCtx,
    trace: Arc<std::sync::Mutex<Vec<serde_json::Value>>>,
}

impl Dispatcher {
    fn push_trace(&self, b: &ToolBinding, status: &str, dur_ms: u64) {
        if let Ok(mut t) = self.trace.lock() {
            t.push(serde_json::json!({
                "tool": b.js_name,
                "server": b.server_name,
                "status": status,
                "duration_ms": dur_ms,
            }));
        }
    }

    /// One host-function call. Returns `{ "value": ... }` on success or
    /// `{ "__error": "..." }` (thrown into the script).
    async fn dispatch_one(&self, js_name: String, args: serde_json::Value) -> serde_json::Value {
        let Some(binding) = self.bindings.iter().find(|b| b.js_name == js_name).cloned() else {
            return serde_json::json!({ "__error": format!("unknown tool '{js_name}'") });
        };

        if !self.budget.try_claim() {
            return serde_json::json!({
                "__error": format!("run_js tool-call budget exhausted (max {})", self.budget.max())
            });
        }

        // Gate exactly like the normal after_llm_call classification.
        let is_builtin =
            crate::modules::mcp::chat_extension::mcp::is_builtin_server_id(binding.server_id);
        let is_control_mutating = binding.server_id
            == crate::modules::control_mcp::control_mcp_server_id()
            && crate::modules::control_mcp::handlers::control_call_needs_approval(
                &binding.tool_name,
                &args,
            );
        let is_auto = self
            .auto_approved
            .contains(&(binding.server_id, binding.tool_name.clone()));

        match approval::gate(is_builtin, is_control_mutating, self.approval_mode.clone(), is_auto) {
            GateDecision::Deny => {
                self.push_trace(&binding, "denied", 0);
                return serde_json::json!({
                    "__error": "MCP tools are disabled for this conversation"
                });
            }
            GateDecision::NeedApproval => {
                match approval::request_approval(
                    &self.approval_ctx,
                    &binding.server_name,
                    &binding.tool_name,
                    &args,
                )
                .await
                {
                    ApprovalOutcome::Approved => {}
                    ApprovalOutcome::Denied(msg) => {
                        self.push_trace(&binding, "denied", 0);
                        return serde_json::json!({ "__error": msg });
                    }
                }
            }
            GateDecision::Allow => {}
        }

        // Dispatch through the chokepoint (records with source=script).
        let t0 = Instant::now();
        let synthetic_id = Uuid::new_v4().to_string();
        let session_arc = match self
            .session_manager
            .get_or_create_with_context(
                binding.server_id,
                self.user_id,
                Some(self.conversation_id),
                Some(self.branch_id),
                self.message_id,
                Some(synthetic_id),
                McpToolCallSource::Script,
            )
            .await
        {
            Ok(a) => a,
            Err(e) => {
                self.push_trace(&binding, "error", t0.elapsed().as_millis() as u64);
                return serde_json::json!({ "__error": format!("dispatch failed: {e}") });
            }
        };

        let (result, _is_final) = {
            let mut session = session_arc.write().await;
            // sse_tx=None → no intermediate tool card; elicit_notify_tx=None → no
            // nested elicitation UI. Result stays in the script.
            execute_tool(
                &mut session,
                &binding.tool_name,
                args,
                &binding.server_name,
                None,
                self.message_id,
                None,
                None,
            )
            .await
        };
        let dur = t0.elapsed().as_millis() as u64;

        if let McpContentData::ToolResult { content, is_error, structured_content, .. } = result {
            let is_err = is_error.unwrap_or(false);
            self.push_trace(&binding, if is_err { "failed" } else { "completed" }, dur);
            serde_json::json!({
                "value": {
                    "content": content,
                    "structuredContent": structured_content,
                    "isError": is_err,
                }
            })
        } else {
            self.push_trace(&binding, "completed", dur);
            serde_json::json!({ "value": null })
        }
    }
}

/// Run one `run_js` script and produce the tool result. Never panics: any
/// interpreter/dispatch failure becomes an error result the model can read.
pub async fn run(req: JsToolRun, script: &str) -> McpContentData {
    let bindings = host_bridge::build_bindings(&req.tools);
    let cancel = Arc::new(AtomicBool::new(false));
    let pending = Arc::new(AtomicUsize::new(0));

    let dispatcher = Arc::new(Dispatcher {
        session_manager: req.session_manager.clone(),
        user_id: req.user_id,
        conversation_id: req.conversation_id,
        branch_id: req.branch_id,
        message_id: req.message_id,
        bindings: bindings.clone(),
        approval_mode: req.approval_mode.clone(),
        auto_approved: req.auto_approved.clone(),
        budget: CallBudget::new(req.caps.max_tool_calls),
        approval_ctx: ApprovalCtx {
            user_id: req.user_id,
            sse_tx: req.sse_tx.clone(),
            pending: pending.clone(),
            timeout: req.caps.approval_timeout,
        },
        trace: Arc::new(std::sync::Mutex::new(Vec::new())),
    });

    // Active-execution wall-clock watchdog: accumulates elapsed time ONLY while
    // no approval is pending, so a long approval-wait never counts. Trips the
    // shared cancel flag, which the runtime's interrupt handler observes.
    let watchdog = {
        let cancel = cancel.clone();
        let pending = pending.clone();
        let wall_ms = req.caps.wall.as_millis() as u64;
        tokio::spawn(async move {
            let mut active_ms: u64 = 0;
            loop {
                tokio::time::sleep(Duration::from_millis(100)).await;
                if cancel.load(Ordering::Relaxed) {
                    break;
                }
                if pending.load(Ordering::SeqCst) == 0 {
                    active_ms += 100;
                    if active_ms >= wall_ms {
                        cancel.store(true, Ordering::Relaxed);
                        break;
                    }
                }
            }
        })
    };

    // The dispatch closure injected as `__ziee_dispatch`.
    let dispatch_fn: DispatchFn = {
        let d = dispatcher.clone();
        Arc::new(move |name, args| {
            let d = d.clone();
            Box::pin(async move { d.dispatch_one(name, args).await })
        })
    };

    let bindings_for_inject = bindings.clone();
    let inject = move |ctx: &rquickjs::Ctx<'_>| {
        host_bridge::install(ctx, &bindings_for_inject, dispatch_fn.clone())
    };

    let outcome = runtime::evaluate(script, &req.caps.runtime, cancel.clone(), inject).await;

    // Stop the watchdog.
    cancel.store(true, Ordering::Relaxed);
    watchdog.abort();

    let trace = dispatcher.trace.lock().map(|t| t.clone()).unwrap_or_default();
    build_result(&req.tool_use_id, outcome, trace)
}

/// Assemble the `run_js` tool result. Only the final value reaches the model's
/// `content` channel; the console + per-tool trace live in `structured_content`.
fn build_result(
    tool_use_id: &str,
    outcome: runtime::JsOutcome,
    trace: Vec<serde_json::Value>,
) -> McpContentData {
    let is_error = outcome.error.is_some();
    let (content, structured) = match &outcome.error {
        Some(err) => {
            let digest = match err.line {
                Some(line) => format!("run_js error (line {line}): {}", err.message),
                None => format!("run_js error: {}", err.message),
            };
            let structured = serde_json::json!({
                "result": null,
                "console": outcome.console,
                "tool_calls": trace,
                "error": { "message": err.message, "line": err.line },
            });
            (digest, structured)
        }
        None => {
            // The model reads the final value as JSON text; the trace/console are
            // inspectable via structured_content / get_tool_result.
            let digest = serde_json::to_string(&outcome.value)
                .unwrap_or_else(|_| "null".to_string());
            let structured = serde_json::json!({
                "result": outcome.value,
                "console": outcome.console,
                "tool_calls": trace,
                "truncated_output": outcome.truncated_output,
            });
            (digest, structured)
        }
    };

    McpContentData::ToolResult {
        tool_use_id: tool_use_id.to_string(),
        name: Some("run_js".to_string()),
        server_id: Some(super::run_js_mcp_server_id().to_string()),
        content,
        is_error: Some(is_error),
        attachment: None,
        images: None,
        resource_links: None,
        hidden_content: None,
        structured_content: Some(structured),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::js_tool::runtime::{JsError, JsOutcome};

    // Result assembly: success → final value in content, trace in structured.
    #[test]
    fn test_build_result_success_shape() {
        let outcome = JsOutcome {
            value: serde_json::json!({ "summary": "ok", "count": 3 }),
            console: vec!["hi".into()],
            error: None,
            truncated_output: false,
        };
        let trace = vec![serde_json::json!({ "tool": "web_search", "status": "completed" })];
        let r = build_result("tu_1", outcome, trace);
        if let McpContentData::ToolResult { content, is_error, structured_content, name, .. } = r {
            assert_eq!(name.as_deref(), Some("run_js"));
            assert_eq!(is_error, Some(false));
            assert!(content.contains("summary"));
            let sc = structured_content.unwrap();
            assert_eq!(sc["result"]["count"], 3);
            assert_eq!(sc["tool_calls"][0]["tool"], "web_search");
        } else {
            panic!("expected ToolResult");
        }
    }

    // Result assembly: error → is_error + line surfaced for self-correction.
    #[test]
    fn test_build_result_error_shape() {
        let outcome = JsOutcome {
            value: serde_json::Value::Null,
            console: vec![],
            error: Some(JsError { message: "boom".into(), line: Some(4) }),
            truncated_output: false,
        };
        let r = build_result("tu_2", outcome, vec![]);
        if let McpContentData::ToolResult { content, is_error, structured_content, .. } = r {
            assert_eq!(is_error, Some(true));
            assert!(content.contains("line 4"));
            assert_eq!(structured_content.unwrap()["error"]["line"], 4);
        } else {
            panic!("expected ToolResult");
        }
    }
}
