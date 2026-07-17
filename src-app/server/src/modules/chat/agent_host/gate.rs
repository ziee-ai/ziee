//! Chat agent-host ports: **cross-request tool approval** (re-home wave 5).
//!
//! Two `agent_core` port impls that reproduce chat's *existing* approval
//! behaviour on top of the shared loop:
//!
//! - [`ChatApprovalPolicy`] (`agent_core::ApprovalPolicy`) — the pure
//!   `Decision` matrix mirroring `mcp/chat_extension/mcp.rs`'s classification
//!   loop (built-in bypass → Disabled deny → per-conversation disabled → control
//!   name/input rule → approval-mode + auto-approve list → unattended allow-list).
//!   Chat drives a **human gate**, not the reviewer, so a mutating call resolves
//!   to [`Decision::Prompt`] where the workflow twin would `Review`.
//! - [`ChatHumanGate`] (`agent_core::HumanGate`) — persists a pending
//!   `tool_use_approvals` row, emits the `McpApprovalRequired` SSE frame, and
//!   returns [`GateOutcome::Suspended`] so the crate loop **ends the turn**. It
//!   never blocks awaiting a human.
//!
//! # Phase-5 UX walk
//! The model requests a mutating/third-party tool. `ChatApprovalPolicy::decide`
//! returns `Prompt`; the loop calls `ChatHumanGate::request`, which writes ONE
//! pending row into `tool_use_approvals` (status `pending`, keyed to
//! `tool_use_id` + this turn's assistant `message_id` + `branch_id`) and pushes
//! an `McpApprovalRequired` SSE frame to the browser. `request` returns
//! `Suspended`, the loop stops, and the chat host finalises the turn (the crate
//! emits `GateOpened`, the host maps it to `ExtensionAction::Complete`). The user
//! sees the approval card and clicks Approve. That is delivered **out of band** by
//! a brand-new `POST .../messages` carrying `tool_approvals`: `before_llm_call`
//! flips the row to `approved`/`denied`, and the resumed turn's first step reads
//! the approved rows and runs the tools — byte-for-byte the flow chat has today.
//!
//! # Phase-5 infra-integration walk (touch list + invariants)
//! - **`tool_use_approvals` table** (cols `tool_use_id, tool_name, tool_input,
//!   server_id, server_name, status, branch_id, conversation_id, message_id,
//!   approved_at, approved_by, approval_note`). Written here via
//!   `Repos.chat.mcp.create_tool_approvals`; FKs cascade with the conversation /
//!   branch / message.
//! - **`McpApprovalRequired` SSE frame** — the single client signal that an
//!   approval is pending; a dropped frame is fatal (there is no other way for the
//!   user to act), which is why `send_approval_required_event` returns `Err` on a
//!   closed channel.
//! - **Cross-request resume is HOST-owned (DEC-22)** — this gate does NOT read
//!   approved rows or execute tools. The message-lifecycle host owns that (see
//!   the HANDOFF recipe): claim-then-execute with `delete_tool_approval`
//!   **before** dispatch is the single-use invariant (a losing concurrent pass
//!   sees `false` and must not run); `cancel_pending_approvals_for_branch` is the
//!   new-turn invariant (a fresh message cancels stale pending rows so an old
//!   prompt can't be answered against a diverged branch).
//! - **Disabled-server gate** — a per-conversation `DisabledServer` entry (or
//!   whole-conversation `ApprovalMode::Disabled`) maps a non-built-in call to
//!   `Deny`, never a prompt.
//! - **Approval-bypass list** — the built-in read-only servers
//!   (`is_builtin_server_id`) auto-approve; the policy relies on the
//!   `ToolProvider::is_trusted` (`trusted`) input carrying that verdict (see
//!   HANDOFF parity note).

use async_trait::async_trait;
use axum::response::sse::Event;
use std::convert::Infallible;
use tokio::sync::mpsc::UnboundedSender;
use uuid::Uuid;

use agent_core::{
    ApprovalPolicy, Decision, GateAsk, GateOutcome, GateTicket, HumanGate, SandboxMode, ToolCall,
};

use crate::common::AppError;
use crate::modules::mcp::chat_extension::approval::models::{
    ApprovalMode, AutoApprovedServer, DisabledServer,
};
use crate::modules::mcp::chat_extension::approval::repository::NewToolApproval;
use crate::modules::mcp::chat_extension::helpers::send_approval_required_event;

// ============================================================
// Shared: recover (server_id, server_str, bare tool_name) from a ToolCall
// ============================================================

/// Chat namespaces a tool as `<server_id>__<tool>` (the `server_id__name` scheme
/// the workflow twin references). The crate leaves `ToolCall.server = None`, so
/// prefer an explicit `call.server` when the host DOES set it, else split the
/// namespaced `call.name` on the FIRST `__`.
///
/// Returns `(parsed server uuid if any, the raw server string, bare tool name)`.
fn split_server_tool(call: &ToolCall) -> (Option<Uuid>, String, String) {
    let (server_str, tool_name) = match &call.server {
        Some(s) => (s.clone(), call.name.clone()),
        None => match call.name.find("__") {
            Some(idx) => (
                call.name[..idx].to_string(),
                call.name[idx + 2..].to_string(),
            ),
            None => (String::new(), call.name.clone()),
        },
    };
    let server_id = Uuid::parse_str(&server_str).ok();
    (server_id, server_str, tool_name)
}

/// Is `(server_id, tool_name)` in an unattended run's allow-list? Mirrors the
/// (module-private) `mcp.rs::unattended_tool_allowed`: the list is a JSON array
/// of `{ server_id, tool_name? }` (`tool_name` absent ⇒ whole server allowed).
fn unattended_tool_allowed(allow: &serde_json::Value, server_str: &str, tool_name: &str) -> bool {
    allow
        .as_array()
        .map(|arr| {
            arr.iter().any(|g| {
                g.get("server_id").and_then(|v| v.as_str()) == Some(server_str)
                    && g.get("tool_name")
                        .and_then(|v| v.as_str())
                        .map(|t| t == tool_name)
                        .unwrap_or(true)
            })
        })
        .unwrap_or(false)
}

// ============================================================
// ChatApprovalPolicy (agent_core::ApprovalPolicy)
// ============================================================

/// Chat's cross-request approval matrix. Constructed once per turn by the chat
/// host from the resolved conversation MCP settings (same reads
/// `mcp.rs::after_llm_call` does), then handed to the loop as `Arc<dyn
/// ApprovalPolicy>`.
pub struct ChatApprovalPolicy {
    pub user_id: Uuid,
    pub conversation_id: Uuid,
    pub branch_id: Uuid,
    /// The conversation's effective approval mode (override → user default →
    /// `ManualApprove`), exactly as resolved in `mcp.rs`.
    pub approval_mode: ApprovalMode,
    /// Per-conversation auto-approved (server, tools) entries.
    pub conv_auto_approved: Vec<AutoApprovedServer>,
    /// User-default auto-approved entries (checked alongside the conversation's).
    pub user_auto_approved: Vec<AutoApprovedServer>,
    /// Per-conversation disabled servers/tools (empty tools ⇒ whole server off).
    pub disabled_servers: Vec<DisabledServer>,
    /// ITEM-13: this is an unattended (scheduled) run — no human can answer a
    /// prompt, so an approval-required call is resolved by `unattended_allowed`.
    pub unattended: bool,
    /// The unattended allow-list (`[{server_id, tool_name?}]`).
    pub unattended_allowed: serde_json::Value,
}

/// Resolve the conversation's effective approval policy for a turn — the SAME
/// resolution `mcp.rs::after_llm_call` performs: conversation MCP settings take
/// precedence, else the user's `/api/mcp/defaults`, else conservative
/// `ManualApprove`. Auto-approved sets are the UNION of the conversation's and the
/// user's. Interactive chat sends are attended (`unattended = false`; the scheduled
/// path is a separate host).
pub async fn resolve_chat_approval_policy(
    user_id: Uuid,
    conversation_id: Uuid,
    branch_id: Uuid,
) -> Result<ChatApprovalPolicy, AppError> {
    use crate::core::Repos;

    let settings = Repos
        .chat
        .mcp
        .get_conversation_settings(conversation_id)
        .await?;
    let user_defaults = {
        use crate::modules::mcp::chat_extension::defaults::repository as defaults_repo;
        defaults_repo::get_user_defaults(Repos.pool(), user_id)
            .await
            .ok()
            .flatten()
    };
    let user_auto_approved = user_defaults
        .as_ref()
        .map(|d| d.get_auto_approved_tools())
        .unwrap_or_default();

    let (approval_mode, conv_auto_approved, disabled_servers) = if let Some(ref s) = settings {
        (
            s.get_approval_mode(),
            s.get_auto_approved_tools(),
            s.get_disabled_servers(),
        )
    } else if let Some(ref d) = user_defaults {
        (
            d.get_approval_mode(),
            d.get_auto_approved_tools(),
            d.get_disabled_servers(),
        )
    } else {
        (ApprovalMode::ManualApprove, Vec::new(), Vec::new())
    };

    Ok(ChatApprovalPolicy {
        user_id,
        conversation_id,
        branch_id,
        approval_mode,
        conv_auto_approved,
        user_auto_approved,
        disabled_servers,
        unattended: false,
        unattended_allowed: serde_json::Value::Null,
    })
}

impl ChatApprovalPolicy {
    /// Is this specific (server, tool) auto-approved by the conversation or the
    /// user defaults? (Mirrors `mcp.rs`'s `is_auto_approved` check.)
    fn is_auto_approved(&self, server_id: Uuid, tool_name: &str) -> bool {
        self.conv_auto_approved
            .iter()
            .any(|s| s.server_id == server_id && s.contains_tool(tool_name))
            || self
                .user_auto_approved
                .iter()
                .any(|s| s.server_id == server_id && s.contains_tool(tool_name))
    }

    /// Is this (server, tool) disabled for the conversation?
    fn is_disabled(&self, server_id: Uuid, tool_name: &str) -> bool {
        self.disabled_servers
            .iter()
            .any(|d| d.server_id == server_id && d.is_tool_disabled(tool_name))
    }

    /// The pure, testable decision (no async / DB). `trusted` is the
    /// `ToolProvider::is_trusted` verdict — for chat parity it MUST equal
    /// `is_builtin_server_id(server_id)` (see the HANDOFF parity note).
    fn decide_pure(
        &self,
        server_id: Option<Uuid>,
        server_str: &str,
        tool_name: &str,
        input: &serde_json::Value,
        trusted: bool,
    ) -> Decision {
        // (1) Built-in read-only / approval-bypassed server → auto-run.
        if trusted {
            return Decision::Auto;
        }

        // (2) Whole-conversation MCP off → deny every non-built-in call (control
        // included). Built-ins were already handled by `trusted` above. This
        // matches mcp.rs's `tools_disabled` (Disabled + !is_builtin) branch.
        if matches!(self.approval_mode, ApprovalMode::Disabled) {
            return Decision::Deny;
        }

        // (3) Per-conversation disabled server/tool → deny (defensive: such tools
        // are normally filtered before being offered).
        if let Some(id) = server_id {
            if self.is_disabled(id, tool_name) {
                return Decision::Deny;
            }
        }

        // (4) Classify needs-approval, mirroring mcp.rs's control / builtin /
        // approval-mode ladder.
        let is_control = server_id
            .map(|id| id == crate::modules::control_mcp::control_mcp_server_id())
            .unwrap_or(false);

        let needs_approval = if is_control {
            // Control is auto-attached but NOT approval-bypassed: read-only
            // control tools auto-run; a mutating `invoke_capability` always needs
            // approval (overriding even AutoApprove).
            crate::modules::control_mcp::handlers::control_call_needs_approval(tool_name, input)
        } else {
            match self.approval_mode {
                ApprovalMode::AutoApprove => false,
                ApprovalMode::ManualApprove => {
                    // Auto-approved for this (server, tool)? Then no prompt.
                    !server_id
                        .map(|id| self.is_auto_approved(id, tool_name))
                        .unwrap_or(false)
                }
                // Handled by the Disabled-deny branch above.
                ApprovalMode::Disabled => return Decision::Deny,
            }
        };

        if !needs_approval {
            return Decision::Auto;
        }

        // (5) An approval-required call. In an unattended run there is no human to
        // ask: an allow-listed tool AUTO-RUNS (pre-authorised by the task
        // creator), a non-allow-listed one is DENIED (no orphaned pending row).
        if self.unattended {
            return if unattended_tool_allowed(&self.unattended_allowed, server_str, tool_name) {
                Decision::Auto
            } else {
                Decision::Deny
            };
        }

        // (6) Chat uses a human gate, not the reviewer → Prompt (not Review).
        Decision::Prompt
    }
}

#[async_trait]
impl ApprovalPolicy for ChatApprovalPolicy {
    async fn decide(&self, call: &ToolCall, trusted: bool, _sandbox: &SandboxMode) -> Decision {
        let (server_id, server_str, tool_name) = split_server_tool(call);
        self.decide_pure(server_id, &server_str, &tool_name, &call.input, trusted)
    }
}

// ============================================================
// ChatHumanGate (agent_core::HumanGate)
// ============================================================

/// Chat's cross-request human gate. Persists ONE pending `tool_use_approvals`
/// row for the call, emits the `McpApprovalRequired` SSE frame, and returns
/// `Suspended` so the crate loop ends the turn. The RESUME (reading approved
/// rows + executing) is owned by the message-lifecycle host (DEC-22) — see the
/// HANDOFF recipe in this file's module doc.
pub struct ChatHumanGate {
    pub user_id: Uuid,
    pub conversation_id: Uuid,
    pub branch_id: Uuid,
    /// The assistant message that carries this turn's `tool_use` blocks — the
    /// pending row's `message_id`, and the key the resume path claims against.
    pub assistant_message_id: Uuid,
    /// The live SSE sink for this request (None for a non-streaming caller).
    pub tx: Option<UnboundedSender<Result<Event, Infallible>>>,
}

#[async_trait]
impl HumanGate for ChatHumanGate {
    async fn request(&self, _run_id: Uuid, ask: GateAsk) -> Result<GateOutcome, AppError> {
        let (server_id, server_str, tool_name) = split_server_tool(&ask.call);

        // Resolve a human-friendly server name for the approval card. Mirrors
        // mcp.rs (name from the server row, else the id/prefix string).
        let server_name = match server_id {
            Some(id) => crate::core::Repos
                .mcp
                .get_any_server(id)
                .await?
                .map(|s| s.name)
                .unwrap_or_else(|| id.to_string()),
            None => server_str.clone(),
        };

        // Persist the pending approval (single row, batch API). tool_use_id =
        // the model's tool_use id (`call.id`); tool_input = the raw args.
        let new_approval = NewToolApproval {
            tool_use_id: ask.call.id.clone(),
            tool_name: tool_name.clone(),
            tool_input: ask.call.input.clone(),
            server_id,
            server_name: server_name.clone(),
        };
        let created = crate::core::Repos
            .chat
            .mcp
            .create_tool_approvals(
                self.conversation_id,
                self.branch_id,
                self.assistant_message_id,
                self.user_id,
                std::slice::from_ref(&new_approval),
            )
            .await?;

        // Emit the client signal (fatal on a closed channel — the user has no
        // other way to act on the pending approval).
        send_approval_required_event(
            self.tx.as_ref(),
            &ask.call.id,
            &tool_name,
            &server_name,
            &server_str,
            &ask.call.input,
        )
        .await?;

        // The ticket id is the pending row's id (falls back to a fresh uuid if
        // the insert somehow returned nothing). Suspended → the loop ends the
        // turn; the host parks and resumes on the next request.
        let ticket_id = created.first().map(|a| a.id).unwrap_or_else(Uuid::new_v4);
        Ok(GateOutcome::Suspended(GateTicket { id: ticket_id }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn policy(mode: ApprovalMode) -> ChatApprovalPolicy {
        ChatApprovalPolicy {
            user_id: Uuid::new_v4(),
            conversation_id: Uuid::new_v4(),
            branch_id: Uuid::new_v4(),
            approval_mode: mode,
            conv_auto_approved: vec![],
            user_auto_approved: vec![],
            disabled_servers: vec![],
            unattended: false,
            unattended_allowed: serde_json::json!([]),
        }
    }

    const SANDBOX: SandboxMode = SandboxMode::WorkspaceWrite { network: true };

    #[test]
    fn trusted_always_auto_regardless_of_mode() {
        let sid = Uuid::new_v4();
        for mode in [
            ApprovalMode::Disabled,
            ApprovalMode::AutoApprove,
            ApprovalMode::ManualApprove,
        ] {
            let p = policy(mode);
            assert_eq!(
                p.decide_pure(Some(sid), &sid.to_string(), "read_file", &serde_json::json!({}), true),
                Decision::Auto
            );
        }
    }

    #[test]
    fn disabled_mode_denies_non_builtin() {
        let sid = Uuid::new_v4();
        let p = policy(ApprovalMode::Disabled);
        assert_eq!(
            p.decide_pure(Some(sid), &sid.to_string(), "do_thing", &serde_json::json!({}), false),
            Decision::Deny
        );
    }

    #[test]
    fn auto_approve_mode_runs_without_prompt() {
        let sid = Uuid::new_v4();
        let p = policy(ApprovalMode::AutoApprove);
        assert_eq!(
            p.decide_pure(Some(sid), &sid.to_string(), "do_thing", &serde_json::json!({}), false),
            Decision::Auto
        );
    }

    #[test]
    fn manual_approve_prompts_unless_auto_approved() {
        let sid = Uuid::new_v4();
        let mut p = policy(ApprovalMode::ManualApprove);
        // Not on the auto-approve list → Prompt.
        assert_eq!(
            p.decide_pure(Some(sid), &sid.to_string(), "do_thing", &serde_json::json!({}), false),
            Decision::Prompt
        );
        // Auto-approved for this (server, tool) → Auto.
        p.conv_auto_approved = vec![AutoApprovedServer {
            server_id: sid,
            tools: vec!["do_thing".to_string()],
        }];
        assert_eq!(
            p.decide_pure(Some(sid), &sid.to_string(), "do_thing", &serde_json::json!({}), false),
            Decision::Auto
        );
    }

    #[test]
    fn disabled_server_denies() {
        let sid = Uuid::new_v4();
        let mut p = policy(ApprovalMode::ManualApprove);
        p.disabled_servers = vec![DisabledServer {
            server_id: sid,
            tools: vec![], // whole server disabled
        }];
        assert_eq!(
            p.decide_pure(Some(sid), &sid.to_string(), "do_thing", &serde_json::json!({}), false),
            Decision::Deny
        );
    }

    #[test]
    fn unattended_allow_list_gates_prompt() {
        let sid = Uuid::new_v4();
        let mut p = policy(ApprovalMode::ManualApprove);
        p.unattended = true;
        // Not allow-listed → Deny (no orphaned pending row).
        assert_eq!(
            p.decide_pure(Some(sid), &sid.to_string(), "do_thing", &serde_json::json!({}), false),
            Decision::Deny
        );
        // Allow-listed → Auto (pre-authorised by the task creator).
        p.unattended_allowed = serde_json::json!([{ "server_id": sid.to_string(), "tool_name": "do_thing" }]);
        assert_eq!(
            p.decide_pure(Some(sid), &sid.to_string(), "do_thing", &serde_json::json!({}), false),
            Decision::Auto
        );
    }

    #[tokio::test]
    async fn async_decide_splits_namespaced_name() {
        let sid = Uuid::new_v4();
        let p = policy(ApprovalMode::ManualApprove);
        // No explicit `server`, namespaced name `<server_id>__tool`.
        let call = ToolCall {
            id: "tu_1".into(),
            server: None,
            name: format!("{sid}__do_thing"),
            input: serde_json::json!({}),
        };
        assert_eq!(p.decide(&call, false, &SANDBOX).await, Decision::Prompt);
    }
}
