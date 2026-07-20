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
/// Recover the `server_id` for a BARE tool name — a model that returned `<tool>`
/// with no `<server>__` prefix. Finds which of the user's accessible servers
/// advertises a tool by this name (first match wins), listing on demand. Mirrors
/// the legacy `recover_server_id_for_bare_name` (which pre-builds the bare-name→id
/// map in `before_llm_call`); here it runs only at gate time when the prefix was
/// absent, so the persisted approval row carries a usable id for the resume path.
async fn resolve_bare_tool_server(user_id: Uuid, tool_name: &str) -> Option<Uuid> {
    let manager = crate::modules::mcp::client::manager::global()?;
    let servers = crate::modules::mcp::chat_extension::helpers::get_all_accessible_config(
        crate::core::Repos.pool(),
        user_id,
    )
    .await
    .ok()?;
    // AMBIGUITY GUARD (parity with legacy `recover_server_id_for_bare_name`): a bare
    // tool name advertised by MORE THAN ONE accessible server is UNRESOLVABLE — never
    // guess, or a bare-name approval could persist the wrong server_id and the resume
    // would execute the approved args against a different server than intended. Collect
    // ALL matches; resolve ONLY when exactly one server advertises the name.
    let mut matches: Vec<Uuid> = Vec::new();
    for s in servers {
        if !s.enabled {
            continue;
        }
        if let Ok(session) = manager
            .get_or_create_with_context(
                s.id,
                user_id,
                None,
                None,
                None,
                None,
                crate::modules::mcp::tool_calls::models::McpToolCallSource::Chat,
            )
            .await
        {
            let listed = {
                let mut g = session.write().await;
                g.list_tools().await
            };
            if let Ok(tools) = listed {
                if tools.iter().any(|t| t.name == tool_name) {
                    matches.push(s.id);
                    if matches.len() > 1 {
                        return None; // ambiguous → do not guess
                    }
                }
            }
        }
    }
    matches.into_iter().next()
}

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
    /// ITEM-54/DEC-112 + FINDING-2: admin per-(server, tool) approval overrides,
    /// batch-loaded once per turn (same `get_tool_approval_overrides_for_servers`
    /// the legacy classification loop uses). An override WINS over the built-in
    /// bypass / control force-approval / conversation default — otherwise the
    /// agent-core chat path (`ZIEE_CHAT_AGENT_CORE=1`) dropped it entirely.
    pub admin_tool_overrides:
        std::collections::HashMap<Uuid, std::collections::HashMap<String, ApprovalMode>>,
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

    // ITEM-54/DEC-112 + FINDING-2: batch-load the admin per-(server, tool)
    // approval overrides for every server that could be called this turn — the
    // user's accessible servers PLUS every auto-attached built-in (whose ids are
    // deterministic and NOT always returned by `get_all_accessible_config`) PLUS
    // control/background. The override query filters non-empty rows in SQL, so a
    // broad id set is cheap. Propagated with `?` (a failed admin-policy read must
    // not fail-open into "no override").
    let override_ids: Vec<Uuid> = {
        let mut ids: Vec<Uuid> =
            crate::modules::mcp::chat_extension::mcp::builtin_server_ids().to_vec();
        ids.push(crate::modules::control_mcp::control_mcp_server_id());
        ids.push(crate::modules::background_mcp::background_mcp_server_id());
        if let Ok(accessible) = crate::modules::mcp::chat_extension::helpers::get_all_accessible_config(
            Repos.pool(),
            user_id,
        )
        .await
        {
            ids.extend(accessible.iter().map(|s| s.id));
        }
        ids
    };
    let admin_tool_overrides = Repos
        .mcp
        .get_tool_approval_overrides_for_servers(&override_ids)
        .await?;

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
        admin_tool_overrides,
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

    /// Look up the admin per-(server, tool) approval override for this call, if
    /// any (ITEM-54/DEC-112 + FINDING-2). `None` server_id (a bare tool name that
    /// hasn't been resolved) or no override → `None`.
    fn admin_override(&self, server_id: Option<Uuid>, tool_name: &str) -> Option<&ApprovalMode> {
        server_id
            .and_then(|id| self.admin_tool_overrides.get(&id))
            .and_then(|m| m.get(tool_name))
    }

    /// The pure, testable decision (no async / DB). `trusted` is the
    /// `ToolProvider::is_trusted` verdict — for chat parity it MUST equal
    /// `is_builtin_server_id(server_id)` (see the HANDOFF parity note).
    /// `admin_override` is the resolved per-(server, tool) admin override for this
    /// call ([`Self::admin_override`]) — it WINS over the built-in bypass, the
    /// control force-approval, and the conversation default (same precedence as
    /// the legacy classification loop).
    fn decide_pure(
        &self,
        server_id: Option<Uuid>,
        server_str: &str,
        tool_name: &str,
        input: &serde_json::Value,
        trusted: bool,
        admin_override: Option<&ApprovalMode>,
    ) -> Decision {
        // (0) Whole-conversation MCP off → deny every non-built-in call (control
        // included), BEFORE the admin override. Mirrors mcp.rs, where the
        // `tools_disabled` (Disabled + !is_builtin) deny precedes the override
        // consult; a built-in (`trusted`) is NOT denied here (it falls through so
        // its override, or bypass, is honored).
        if !trusted && matches!(self.approval_mode, ApprovalMode::Disabled) {
            return Decision::Deny;
        }

        // (1) Admin per-(server, tool) approval override (ITEM-54/DEC-112 +
        // FINDING-2). Consulted BEFORE the built-in bypass, the control
        // force-approval, and the conversation default — an override on ANY
        // server (incl. a built-in / control) WINS. Absent → fall through to the
        // historical classification unchanged. (DRIFT-1.4: an explicit
        // auto_approve on a mutating built-in is the admin's own deliberate
        // footgun, honored — not the silent ignore this fixes.)
        let needs_approval = if let Some(mode) = admin_override {
            match mode {
                ApprovalMode::Disabled => return Decision::Deny,
                ApprovalMode::AutoApprove => false,
                ApprovalMode::ManualApprove => true,
            }
        } else {
            // ── historical no-override classification ──
            // (2) Built-in read-only / approval-bypassed server → auto-run.
            if trusted {
                return Decision::Auto;
            }

            // (3) Per-conversation disabled server/tool → deny (defensive: such
            // tools are normally filtered before being offered).
            if let Some(id) = server_id {
                if self.is_disabled(id, tool_name) {
                    return Decision::Deny;
                }
            }

            // (4) Classify needs-approval, mirroring mcp.rs's control /
            // approval-mode ladder.
            let is_control = server_id
                .map(|id| id == crate::modules::control_mcp::control_mcp_server_id())
                .unwrap_or(false);

            if is_control {
                // Control is auto-attached but NOT approval-bypassed: read-only
                // control tools auto-run; a mutating `invoke_capability` always
                // needs approval (overriding even AutoApprove).
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
                    // Handled by the Disabled-deny branch above (non-builtin);
                    // unreachable here.
                    ApprovalMode::Disabled => return Decision::Deny,
                }
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
        // Cross-request resume: if a prior turn already recorded a human decision
        // for THIS tool_use_id (the row was flipped by the MCP extension's
        // before_llm_call on the resuming request), honor it directly instead of
        // re-classifying — else a manual-approve tool would re-prompt forever.
        use crate::modules::mcp::chat_extension::approval::repository as approval_repo;
        let pool = crate::core::Repos.pool();
        if let Ok(approved) =
            approval_repo::get_approved_tools_for_branch(pool, self.branch_id).await
        {
            if let Some(row) = approved.iter().find(|a| a.tool_use_id == call.id) {
                // Single-use CLAIM: delete the row and Auto-run ONLY if WE won the
                // delete (Ok(true)). A `false`/`Err` means a concurrent pass already
                // claimed it (or the read/delete failed) — do NOT auto-execute a
                // possibly-double-run mutating tool; fall through to re-classify
                // (→ Prompt for a manual tool), never a silent second execution.
                if matches!(
                    approval_repo::delete_tool_approval(pool, row.tool_use_id.clone(), row.message_id)
                        .await,
                    Ok(true)
                ) {
                    return Decision::Auto;
                }
            }
        }
        // The denial check is a SECURITY gate: a query failure must NOT be read as
        // "not denied" (fail-open). On a denial-read error, escalate to a human
        // (Prompt) rather than risk auto-executing a denied tool.
        match approval_repo::get_denied_tools_for_branch(pool, self.branch_id).await {
            Ok(denied) if denied.iter().any(|a| a.tool_use_id == call.id) => {
                return Decision::Deny;
            }
            Ok(_) => {}
            Err(_) => return Decision::Prompt,
        }
        let (server_id, server_str, tool_name) = split_server_tool(call);
        let admin_override = self.admin_override(server_id, &tool_name);
        self.decide_pure(
            server_id,
            &server_str,
            &tool_name,
            &call.input,
            trusted,
            admin_override,
        )
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
        let (mut server_id, server_str, tool_name) = split_server_tool(&ask.call);
        // The namespaced tool prefix may be a server NAME (not a uuid) — e.g. a
        // user-registered server whose tools are advertised as `<name>__<tool>`.
        // Resolve it to the server_id so the PERSISTED approval row carries a
        // usable id; otherwise the resume path (`execute_approved_tools_sync`) hits
        // "No server_id in approval record" and never executes the approved tool.
        if server_id.is_none() && !server_str.is_empty() {
            server_id = crate::modules::mcp::agent_tool_call::resolve_tool_server(self.user_id, &server_str)
                .await
                .ok();
        }
        // BARE tool name (the model returned `<tool>` with no `<server>__` prefix):
        // recover which of the user's accessible servers advertises it — the legacy
        // `recover_server_id_for_bare_name` equivalent, listed on demand at gate time.
        if server_id.is_none() {
            server_id = resolve_bare_tool_server(self.user_id, &tool_name).await;
        }

        // Resolve the server row ONCE: the human-friendly name AND — for a
        // full-disclosure data-egress approval card (ITEM-50) — the external
        // destination host. Mirrors mcp.rs (name from the row, else the
        // id/prefix string).
        let server_row = match server_id {
            Some(id) => crate::core::Repos.mcp.get_any_server(id).await?,
            None => None,
        };
        let server_name = match (&server_row, server_id) {
            (Some(s), _) => s.name.clone(),
            (None, Some(id)) => id.to_string(),
            (None, None) => server_str.clone(),
        };
        // ITEM-50: the EXTERNAL destination host (pure, derived from the already
        // resolved internal row — no new SSRF/host logic) + the tool's FULL exact
        // description (best-effort live `tools/list`). Both `None` for a
        // built-in/loopback server or on failure → the card shows a local call.
        let dest_host = server_row
            .as_ref()
            .and_then(crate::modules::mcp::agent_tool_call::resolve_dest_host);
        let description = crate::modules::mcp::agent_tool_call::resolve_tool_description(
            server_id,
            self.user_id,
            &tool_name,
        )
        .await;

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
            dest_host,
            description,
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
    use crate::modules::mcp::agent_tool_call::resolve_dest_host;
    use crate::modules::mcp::{McpServer, TransportType, UsageMode};

    /// A minimal `McpServer` row for the ITEM-50 dest-host disclosure tests.
    /// Mirrors the `client::stdio` test template; the caller overrides the fields
    /// under test (transport / url / is_built_in / is_system).
    fn server_row(transport: TransportType, url: Option<&str>) -> McpServer {
        McpServer {
            id: Uuid::new_v4(),
            user_id: None,
            name: "acme-remote".into(),
            display_name: "Acme Remote".into(),
            description: Some("An external MCP server".into()),
            enabled: true,
            is_system: false,
            is_built_in: false,
            transport_type: transport,
            command: None,
            args: serde_json::Value::Array(vec![]),
            environment_variables: serde_json::Value::Object(Default::default()),
            environment_variables_entries: Vec::new(),
            url: url.map(String::from),
            headers: serde_json::Value::Object(Default::default()),
            headers_entries: Vec::new(),
            timeout_seconds: 30,
            supports_sampling: false,
            usage_mode: UsageMode::Auto,
            max_concurrent_sessions: None,
            run_in_sandbox: false,
            sandbox_flavor: "full".into(),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            last_health_check_at: None,
            last_health_check_status: "untested".into(),
            last_health_check_reason: None,
        }
    }

    // ---- ITEM-50 / TEST-182: full-disclosure approval-card dest host --------

    #[test]
    fn dest_host_names_external_http_host_only() {
        // The approval card must NAME the concrete destination host so the human
        // reviews a data-egress decision. Only the HOST is returned — never the
        // path/query (no credential/path leak).
        let s = server_row(
            TransportType::Http,
            Some("https://api.example.com/v1/mcp?key=abc"),
        );
        let host = resolve_dest_host(&s).expect("external http host resolved");
        assert_eq!(host, "api.example.com");
        assert!(!host.contains('/'), "host must not carry the URL path");
        assert!(!host.contains("key="), "host must not carry the query string");
    }

    #[test]
    fn dest_host_resolved_server_side_for_is_system_row_without_leaking_redacted_url() {
        // An `is_system` server's `url` is redacted to `None` in the PUBLIC list
        // view, but the gate resolves from the INTERNAL row (`get_any_server`),
        // whose `url` is real — so the host is resolved server-side regardless of
        // the redacted public view.
        let mut s = server_row(TransportType::Sse, Some("https://mcp.internal-corp.net/sse"));
        s.is_system = true;
        let host = resolve_dest_host(&s).expect("is_system external host resolved");
        assert_eq!(host, "mcp.internal-corp.net");
    }

    #[test]
    fn dest_host_none_for_builtin_loopback_and_stdio() {
        // Built-in servers are in-process loopback → no external destination,
        // even with an http url set.
        let mut builtin = server_row(TransportType::Http, Some("http://127.0.0.1:9100/mcp"));
        builtin.is_built_in = true;
        assert_eq!(resolve_dest_host(&builtin), None);

        // A user-registered http server pointed at loopback is not a meaningful
        // EXTERNAL destination either.
        let loopback = server_row(TransportType::Http, Some("http://localhost:8080/mcp"));
        assert_eq!(resolve_dest_host(&loopback), None);

        // stdio: a local subprocess, no network destination.
        let stdio = server_row(TransportType::Stdio, None);
        assert_eq!(resolve_dest_host(&stdio), None);
    }

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
            admin_tool_overrides: std::collections::HashMap::new(),
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
                p.decide_pure(Some(sid), &sid.to_string(), "read_file", &serde_json::json!({}), true, None),
                Decision::Auto
            );
        }
    }

    #[test]
    fn disabled_mode_denies_non_builtin() {
        let sid = Uuid::new_v4();
        let p = policy(ApprovalMode::Disabled);
        assert_eq!(
            p.decide_pure(Some(sid), &sid.to_string(), "do_thing", &serde_json::json!({}), false, None),
            Decision::Deny
        );
    }

    #[test]
    fn auto_approve_mode_runs_without_prompt() {
        let sid = Uuid::new_v4();
        let p = policy(ApprovalMode::AutoApprove);
        assert_eq!(
            p.decide_pure(Some(sid), &sid.to_string(), "do_thing", &serde_json::json!({}), false, None),
            Decision::Auto
        );
    }

    #[test]
    fn manual_approve_prompts_unless_auto_approved() {
        let sid = Uuid::new_v4();
        let mut p = policy(ApprovalMode::ManualApprove);
        // Not on the auto-approve list → Prompt.
        assert_eq!(
            p.decide_pure(Some(sid), &sid.to_string(), "do_thing", &serde_json::json!({}), false, None),
            Decision::Prompt
        );
        // Auto-approved for this (server, tool) → Auto.
        p.conv_auto_approved = vec![AutoApprovedServer {
            server_id: sid,
            tools: vec!["do_thing".to_string()],
        }];
        assert_eq!(
            p.decide_pure(Some(sid), &sid.to_string(), "do_thing", &serde_json::json!({}), false, None),
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
            p.decide_pure(Some(sid), &sid.to_string(), "do_thing", &serde_json::json!({}), false, None),
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
            p.decide_pure(Some(sid), &sid.to_string(), "do_thing", &serde_json::json!({}), false, None),
            Decision::Deny
        );
        // Allow-listed → Auto (pre-authorised by the task creator).
        p.unattended_allowed = serde_json::json!([{ "server_id": sid.to_string(), "tool_name": "do_thing" }]);
        assert_eq!(
            p.decide_pure(Some(sid), &sid.to_string(), "do_thing", &serde_json::json!({}), false, None),
            Decision::Auto
        );
    }

    // ── ITEM-54/DEC-112 + FINDING-2: admin per-(server, tool) override ──
    // The override WINS over the built-in bypass, the control force-approval, and
    // the conversation default (byte-for-byte the legacy loop's precedence). The
    // agent-core chat path previously dropped it entirely.

    #[test]
    fn admin_override_disabled_denies_trusted_builtin() {
        // A built-in (`trusted=true`) normally bypasses to Auto; an admin
        // `disabled` override must deny it instead (the silent-ignore bug fixed).
        let sid = Uuid::new_v4();
        let p = policy(ApprovalMode::AutoApprove);
        assert_eq!(
            p.decide_pure(
                Some(sid),
                &sid.to_string(),
                "read_file",
                &serde_json::json!({}),
                true,
                Some(&ApprovalMode::Disabled),
            ),
            Decision::Deny
        );
    }

    #[test]
    fn admin_override_manual_prompts_trusted_builtin() {
        // A normally-bypassed built-in read becomes approval-required (Prompt)
        // under an admin `manual_approve` override.
        let sid = Uuid::new_v4();
        let p = policy(ApprovalMode::AutoApprove);
        assert_eq!(
            p.decide_pure(
                Some(sid),
                &sid.to_string(),
                "read_file",
                &serde_json::json!({}),
                true,
                Some(&ApprovalMode::ManualApprove),
            ),
            Decision::Prompt
        );
    }

    #[test]
    fn admin_override_changes_conversation_default() {
        let sid = Uuid::new_v4();
        // ManualApprove conversation would Prompt a non-builtin tool …
        let p = policy(ApprovalMode::ManualApprove);
        assert_eq!(
            p.decide_pure(Some(sid), &sid.to_string(), "do_thing", &serde_json::json!({}), false, None),
            Decision::Prompt
        );
        // … but an admin `auto_approve` override runs it without a prompt.
        assert_eq!(
            p.decide_pure(
                Some(sid),
                &sid.to_string(),
                "do_thing",
                &serde_json::json!({}),
                false,
                Some(&ApprovalMode::AutoApprove),
            ),
            Decision::Auto
        );
        // … and an admin `disabled` override denies it even in an AutoApprove
        // conversation (override tightens, wins over the default).
        let p_auto = policy(ApprovalMode::AutoApprove);
        assert_eq!(
            p_auto.decide_pure(
                Some(sid),
                &sid.to_string(),
                "do_thing",
                &serde_json::json!({}),
                false,
                Some(&ApprovalMode::Disabled),
            ),
            Decision::Deny
        );
    }

    #[test]
    fn admin_override_lookup_threads_into_decide() {
        // The map-backed `admin_override` lookup (populated per turn) resolves the
        // mode for a (server, tool) and changes the decision vs the map being empty.
        let sid = Uuid::new_v4();
        let mut p = policy(ApprovalMode::AutoApprove);
        // Empty map → no override → built-in bypass runs.
        assert!(p.admin_override(Some(sid), "read_file").is_none());
        assert_eq!(
            p.decide_pure(
                Some(sid),
                &sid.to_string(),
                "read_file",
                &serde_json::json!({}),
                true,
                p.admin_override(Some(sid), "read_file"),
            ),
            Decision::Auto
        );
        // Populate an override for exactly this (server, tool) → decision flips.
        let mut per_tool = std::collections::HashMap::new();
        per_tool.insert("read_file".to_string(), ApprovalMode::Disabled);
        p.admin_tool_overrides.insert(sid, per_tool);
        assert_eq!(p.admin_override(Some(sid), "read_file"), Some(&ApprovalMode::Disabled));
        assert_eq!(
            p.decide_pure(
                Some(sid),
                &sid.to_string(),
                "read_file",
                &serde_json::json!({}),
                true,
                p.admin_override(Some(sid), "read_file"),
            ),
            Decision::Deny
        );
        // A DIFFERENT tool on the same server is unaffected (still bypasses).
        assert!(p.admin_override(Some(sid), "other_tool").is_none());
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
