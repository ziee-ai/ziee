//! Per-call approval suspend/resume for gated sub-tools invoked INSIDE a
//! `run_js` script.
//!
//! Mechanism (DEC-2/DEC-3): reuse the vetted `mcp::elicitation::registry`
//! in-process oneshot — NOT the `tool_use_approvals` turn-boundary flow, which
//! ends the HTTP request and cannot resume a live QuickJS call stack. We
//! register a oneshot with `content_id = None` (no form content row), bind the
//! owner synchronously (fail-closed), emit a `runJsApprovalRequired` SSE event,
//! and `select!` over {oneshot, stream-closed, timeout}. The user's decision
//! arrives via the EXISTING `POST /api/mcp/elicitation/{id}/respond` endpoint
//! (it tolerates `content_id = None`). `accept` → the sub-tool runs;
//! `decline`/`cancel`/timeout → the host fn throws a catchable error.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use tokio::sync::oneshot;
use uuid::Uuid;

use crate::modules::chat::core::types::streaming::SSEChatStreamEvent;
use crate::modules::mcp::chat_extension::ApprovalMode;
use crate::modules::mcp::chat_extension::extension::SSEChatStreamRunJsApprovalRequiredData;
use crate::modules::mcp::elicitation::{models::ElicitationResponse, registry};

/// SSE channel type threaded through the chat stream.
pub type SseTx =
    tokio::sync::mpsc::UnboundedSender<Result<axum::response::sse::Event, std::convert::Infallible>>;

/// The gate decision for a sub-tool call, mirroring the mcp.rs after_llm_call
/// classification (mcp.rs:1897) but as a pure function so it is unit-testable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GateDecision {
    /// Auto-run (built-in, or auto-approved, or AutoApprove mode).
    Allow,
    /// Suspend the script and prompt the user.
    NeedApproval,
    /// Refuse — MCP is Disabled for this conversation and the tool is not a
    /// built-in. The host fn throws.
    Deny,
}

/// Decide how a sub-tool call is gated. Same rules as the normal loop:
/// - a mutating `control` call ALWAYS needs approval (overrides AutoApprove);
/// - a built-in server bypasses approval;
/// - otherwise the conversation's `ApprovalMode` + the per-tool allowlist decide.
pub fn gate(
    is_builtin: bool,
    is_control_mutating: bool,
    mode: ApprovalMode,
    is_auto_approved: bool,
) -> GateDecision {
    // Order matters and MUST match the normal after_llm_call loop (mcp.rs):
    // 1. Built-in privileged servers always execute — even under Disabled.
    if is_builtin {
        return GateDecision::Allow;
    }
    // 2. Disabled is the kill switch: a non-builtin tool is DENIED outright,
    //    BEFORE any control/approval classification. Without this, a mutating
    //    `control` tool (deliberately NOT a built-in) would get an approval
    //    prompt inside a script even though MCP is off — a confused-deputy
    //    bypass of the Disabled contract (security audit, medium).
    if matches!(mode, ApprovalMode::Disabled) {
        return GateDecision::Deny;
    }
    // 3. A mutating control call always needs approval (overrides AutoApprove).
    if is_control_mutating {
        return GateDecision::NeedApproval;
    }
    match mode {
        ApprovalMode::AutoApprove => GateDecision::Allow,
        ApprovalMode::ManualApprove => {
            if is_auto_approved {
                GateDecision::Allow
            } else {
                GateDecision::NeedApproval
            }
        }
        // Handled above.
        ApprovalMode::Disabled => GateDecision::Deny,
    }
}

/// Outcome of a suspend/resume approval round.
pub enum ApprovalOutcome {
    Approved,
    /// The message thrown into the script as a catchable `ToolApprovalDenied`.
    Denied(String),
}

/// Increments a shared "approvals in flight" counter for its lifetime so the
/// executor's wall-clock watchdog pauses while any approval is pending (the
/// approval-wait must not count toward the active-execution budget).
struct PendingGuard(Arc<AtomicUsize>);
impl PendingGuard {
    fn new(c: Arc<AtomicUsize>) -> Self {
        c.fetch_add(1, Ordering::SeqCst);
        Self(c)
    }
}
impl Drop for PendingGuard {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}

/// Shared context for suspending on approvals during one script run.
pub struct ApprovalCtx {
    pub user_id: Uuid,
    pub sse_tx: SseTx,
    /// Shared counter the executor's watchdog reads to pause the wall-clock.
    pub pending: Arc<AtomicUsize>,
    pub timeout: Duration,
}

/// Suspend the script in-process awaiting the user's decision on one sub-tool
/// call. Concurrent approvals are supported: each gets its own random id, so a
/// `Promise.all` of gated calls surfaces independent prompts.
pub async fn request_approval(
    ctx: &ApprovalCtx,
    server_name: &str,
    tool_name: &str,
    input: &serde_json::Value,
) -> ApprovalOutcome {
    let id = Uuid::new_v4();
    let (tx, rx) = oneshot::channel::<ElicitationResponse>();
    registry::register(id, tx, None);
    // Owner is known synchronously here (unlike ask_user, which binds via the
    // notify listener) — bind immediately so owner_matches is fail-closed.
    registry::bind_owner(id, ctx.user_id);

    let event = SSEChatStreamEvent::RunJsApprovalRequired(SSEChatStreamRunJsApprovalRequiredData {
        elicitation_id: id.to_string(),
        tool_name: tool_name.to_string(),
        server: server_name.to_string(),
        input: input.clone(),
    });
    if ctx.sse_tx.send(Ok(event.into())).is_err() {
        registry::remove(id);
        return ApprovalOutcome::Denied("chat stream closed before approval".to_string());
    }

    // Count this approval as in-flight for the duration of the wait.
    let _pending = PendingGuard::new(ctx.pending.clone());
    let response = tokio::select! {
        r = rx => r.ok(),
        _ = ctx.sse_tx.closed() => { registry::remove(id); None }
        _ = tokio::time::sleep(ctx.timeout) => { registry::remove(id); None }
    };

    match response {
        Some(r) if r.action == "accept" => ApprovalOutcome::Approved,
        Some(r) => {
            // Map the action to a correct past-tense verb (not `{action}ed`,
            // which yields "declineed"). Vocabulary is accept|decline|cancel.
            let verb = match r.action.as_str() {
                "decline" => "declined",
                "cancel" => "cancelled",
                other => return ApprovalOutcome::Denied(format!("tool call {other} by user")),
            };
            ApprovalOutcome::Denied(format!("tool call {verb} by user"))
        }
        None => ApprovalOutcome::Denied("tool approval timed out or was cancelled".to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // TEST-14: the gate decision matches the normal loop.
    #[test]
    fn test_gate_decision_matches_normal_loop() {
        // Built-in server → bypass (no prompt), regardless of mode.
        assert_eq!(gate(true, false, ApprovalMode::ManualApprove, false), GateDecision::Allow);
        assert_eq!(gate(true, false, ApprovalMode::Disabled, false), GateDecision::Allow);

        // Control-mutating → always prompt, even under AutoApprove.
        assert_eq!(gate(false, true, ApprovalMode::AutoApprove, true), GateDecision::NeedApproval);

        // ManualApprove, not allowlisted → prompt.
        assert_eq!(gate(false, false, ApprovalMode::ManualApprove, false), GateDecision::NeedApproval);
        // ManualApprove, allowlisted → allow.
        assert_eq!(gate(false, false, ApprovalMode::ManualApprove, true), GateDecision::Allow);

        // AutoApprove non-builtin → allow.
        assert_eq!(gate(false, false, ApprovalMode::AutoApprove, false), GateDecision::Allow);

        // Disabled non-builtin → deny.
        assert_eq!(gate(false, false, ApprovalMode::Disabled, false), GateDecision::Deny);
    }

    #[tokio::test]
    async fn test_stream_closed_denies() {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        drop(rx); // receiver gone → send fails → immediate deny
        let ctx = ApprovalCtx {
            user_id: Uuid::new_v4(),
            sse_tx: tx,
            pending: Arc::new(AtomicUsize::new(0)),
            timeout: Duration::from_secs(1),
        };
        let out = request_approval(&ctx, "srv", "tool", &serde_json::json!({})).await;
        assert!(matches!(out, ApprovalOutcome::Denied(_)));
    }
}
