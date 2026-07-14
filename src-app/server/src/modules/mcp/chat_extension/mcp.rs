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
use crate::modules::mcp::tool_calls::models::{McpCallContext, McpToolCallSource};
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
             get_resource_link returns. These download URLs are SHORT-LIVED: call \
             get_resource_link again to obtain a FRESH URL each time you hand a file to a \
             tool, and never reuse a URL from an earlier turn (an old URL may have stopped \
             working). When another tool HANDS you a file as a URL, use the exact URL ziee \
             gives you for it (an /api/files link, shown as a file-card attachment) — do NOT \
             fetch or forward the tool's raw upstream URL, and NEVER rewrite, guess, or \
             substitute its host (no 127.0.0.1, localhost, or a made-up platform/DRS host).",
        );
    }
    guidance
}

/// Guidance appended (as `hidden_content`) to a tool result whose produced files were saved
/// as durable artifacts, listing the download URLs the model may hand to another tool.
///
/// Those URLs carry SHORT-LIVED download tokens (the 1-hour token minted at the save sites), so
/// they are not durable across turns. The wording therefore tells the model the URLs are
/// temporary and to re-obtain a fresh link via `get_resource_link` rather than reuse one from an
/// earlier turn — the root-cause fix for stale cross-turn artifact references. Pure so the
/// wording is unit-testable (mirrors [`tool_system_guidance`]); the single source of the
/// saved-artifact download guidance (both artifact-save sites call it).
fn saved_artifact_hidden_content_guidance(url_lines: &str) -> String {
    format!(
        "[system: Files saved as artifact attachments (shown as file cards in UI). This includes \
         files another tool returned that ziee has re-hosted for you — use the ziee URL listed \
         below, not the tool's original upstream URL. \
         Do NOT embed file URLs or images inline in your text response. \
         To pass one of these files to another tool, copy its URL below VERBATIM into that \
         tool's file/URL argument — never rewrite the host, never substitute \
         127.0.0.1/localhost, and never invent a DRS or platform URL. These download URLs are \
         TEMPORARY — they may stop working in a later turn: to hand one of these files to a \
         tool again later, first re-obtain a fresh link \
         (for a file you produced in the sandbox, call get_resource_link with its filename) — \
         never reuse a URL from an earlier turn:\n{url_lines}]"
    )
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
    // `citations` attaches behind the flag set by the citations chat extension
    // (`attach_citations_mcp`), gated on tool-capable. Per-user library, always
    // available — no admin enable / provider gate.
    if flag(crate::modules::citations::chat_extension::ATTACH_FLAG) {
        ids.push(crate::modules::citations::citations_server_id());
    }
    // Knowledge base — attaches behind `attach_knowledge_base_mcp` (set only when
    // ≥1 KB is bound to the conversation); read-only search, approval-bypassed.
    if flag(crate::modules::knowledge_base::chat_extension::ATTACH_FLAG) {
        ids.push(crate::modules::knowledge_base::knowledge_base_server_id());
    }
    // `control` attaches behind the flag set by the control chat extension
    // (`attach_control_mcp`), gated on the deploy kill-switch + tool-capable.
    // Unlike the read-only built-ins it is NOT approval-bypassed (see
    // `is_builtin_server_id` — control is intentionally absent); mutating
    // `invoke_capability` calls are forced through approval by the per-tool
    // `control_call_needs_approval` classifier in the approval loop below.
    if flag(crate::modules::control_mcp::chat_extension::ATTACH_FLAG) {
        ids.push(crate::modules::control_mcp::control_mcp_server_id());
    }
    // `skill_mcp` attaches behind the flag set by the skill chat extension
    // (`attach_skill_mcp`), gated on tool-capable + ≥1 available skill. Without
    // this the injected skill listing tells the model to call `load_skill` but
    // the tool is never present.
    if flag(crate::modules::skill::chat_extension::ATTACH_FLAG) {
        ids.push(crate::modules::skill_mcp::skill_mcp_server_id());
    }
    // `run_js` (programmatic tool calling) attaches behind the flag set by the
    // js_tool chat extension (`attach_run_js_mcp`), gated on the deploy kill
    // switch + tool-capable. The model's `run_js` call is approval-bypassed (see
    // `is_builtin_server_id` — the script START auto-runs), while gated sub-tools
    // called INSIDE the script go through per-call approval in the js_tool
    // executor. Execution is intercepted inline (like `ask_user`), not dispatched
    // over the loopback.
    if flag(crate::modules::js_tool::chat_extension::ATTACH_FLAG) {
        ids.push(crate::modules::js_tool::run_js_mcp_server_id());
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

/// Recover the server that advertised a BARE tool name (one the model returned
/// without the `<server_id>__` prefix ziee prepends). `map` is the per-message
/// `bare_name -> Option<server_id>` built in `before_llm_call`; a `None` value
/// marks an ambiguous name advertised by ≥2 servers. Returns `Some(server_id)`
/// ONLY for an unambiguous single-server hit — never guesses, so an ambiguous or
/// unknown name yields `None` and falls through to a clear error instead of
/// mis-dispatching a side-effecting tool.
fn recover_server_id_for_bare_name(
    bare: &str,
    map: &HashMap<String, Option<Uuid>>,
) -> Option<Uuid> {
    map.get(bare).copied().flatten()
}

/// Resolve `(server_id, tool_name)` from a finalized tool-call wire name.
///
/// A well-formed name is `<server_uuid>__<tool>` — split on the FIRST `__` into a
/// valid UUID + the (possibly `__`-containing) tool name. Some models (e.g.
/// gpt-oss/harmony) strip the server prefix, yielding a prefix-less name:
/// bare (`execute_command`), empty-prefix (`__query_rag`), or a bare name that
/// itself contains `__` (`get__weather`). For those, recover the server from
/// `map` (the tools advertised this turn) by trying the whole name, and — ONLY
/// for an empty-prefix `__tool` — the remainder after the leading `__`. A `__`
/// in the MIDDLE of a name is part of the tool name and is never stripped (so a
/// non-advertised `get__weather` can't be mis-dispatched to some other server's
/// `weather` tool). Returns `(None, full_name)` when unresolvable — the caller
/// surfaces a clear error rather than guessing.
fn resolve_server_and_tool(
    full_name: &str,
    map: &HashMap<String, Option<Uuid>>,
) -> (Option<Uuid>, String) {
    if let Some((id, name)) = full_name.split_once("__")
        && let Ok(sid) = Uuid::parse_str(id)
    {
        return (Some(sid), name.to_string());
    }
    let candidates: Vec<&str> = match full_name.strip_prefix("__") {
        Some(rest) if !rest.is_empty() => vec![rest, full_name],
        _ => vec![full_name],
    };
    for cand in candidates {
        if let Some(sid) = recover_server_id_for_bare_name(cand, map) {
            return (Some(sid), cand.to_string());
        }
    }
    (None, full_name.to_string())
}

/// Pick the tool_use id to persist for a finalized tool call, guaranteeing it is
/// non-empty and unique within its assistant message. `provider_id` is the id the
/// provider streamed (may be empty, or a non-unique constant like gpt-oss/harmony's
/// `"tool_use"`); `used` is the set of ids already taken by this message (persisted
/// tool_uses from prior loop iterations + ids assigned earlier in this finalize
/// batch). Mints a fresh `call_<uuid>` iff `provider_id` is empty OR already taken;
/// otherwise keeps `provider_id` so well-behaved providers (Anthropic `toolu_…`,
/// real OpenAI `call_…`) round-trip unchanged. ziee owns both sides of the
/// round-trip (the id it sends back as `tool_call_id` and the tool_result that
/// references it), so replacing a bad id is safe.
fn resolve_unique_tool_use_id(provider_id: &str, used: &std::collections::HashSet<String>) -> String {
    if provider_id.is_empty() || used.contains(provider_id) {
        format!("call_{}", Uuid::new_v4())
    } else {
        provider_id.to_string()
    }
}

/// What a claim attempt (the DELETE of the approval row) authorizes.
///
/// Only `Won` authorizes EXECUTION. Both other outcomes still owe the tool_use an
/// answer: the call site emits an `is_error` result and continues rather than
/// skipping or bailing, because a tool_use left with no result at all is
/// unrecoverable (see the call site's comment).
#[derive(Debug, PartialEq, Eq)]
enum ClaimOutcome {
    /// We deleted the row — we own this execution.
    Won,
    /// Zero rows deleted: a concurrent pass already claimed it and is executing it.
    /// Do NOT run it again.
    AlreadyClaimed,
    /// The DELETE itself failed, so we cannot tell whether the row survives. Do NOT
    /// execute — a double-run of a side-effecting tool is the worse outcome.
    Failed,
}

/// Decide what a claim attempt authorizes, from `delete_tool_approval`'s
/// `Result<bool>` (the bool is `rows_affected() > 0`).
///
/// Split out from the loop so the exactly-once decision — the whole point of
/// claiming before executing — is directly unit-testable. Discarding the bool and
/// branching only on `Err` silently turns `AlreadyClaimed` into `Won`, which is a
/// double-execution.
fn claim_outcome<E>(delete_result: Result<bool, E>) -> ClaimOutcome {
    match delete_result {
        Ok(true) => ClaimOutcome::Won,
        Ok(false) => ClaimOutcome::AlreadyClaimed,
        Err(_) => ClaimOutcome::Failed,
    }
}

/// Fold freshly-executed tool results into an assembled request WITHOUT ever
/// producing two `tool_result` blocks for one `tool_use_id` (which Anthropic
/// rejects: "each tool_use must have a single result").
///
/// A result whose `tool_use_id` ALREADY has a `tool_result` in the request replaces
/// that block IN PLACE; the freshly-executed result is authoritative. Results with
/// no existing block are returned for the caller to push as a User message.
///
/// Why replace in place instead of dropping the old block and appending ours: the
/// existing block is a `synthetic_missing_tool_result` placeholder that
/// `group_assistant_blocks` put in the Tool turn IMMEDIATELY AFTER its tool_use —
/// which is exactly where the provider requires it. Removing it and appending the
/// real result to a trailing User message would satisfy "one result per tool_use"
/// while breaking "a tool_result immediately after the tool_use" — the sibling
/// regression this must not reintroduce. Replacing keeps BOTH invariants and
/// upgrades the placeholder to the real result.
///
/// Reached whenever a batch mixes an approval-EXEMPT built-in with an
/// approval-REQUIRED tool: the built-in's result is persisted at the pause, so on
/// resume `group_assistant_blocks` sees a partially-resolved batch, reads the
/// still-unapproved tool as a permanent gap, and synthesizes a placeholder for it —
/// then this path executes it for real.
///
/// SCOPE — only the CURRENT (last) tool batch is searched, never the whole history.
/// A `tool_use_id` is unique only within one assistant message
/// (`resolve_unique_tool_use_id` seeds its used-set from `WHERE message_id = $1`,
/// and deliberately tolerates gpt-oss/harmony's non-unique constant `"tool_use"`),
/// so an id can recur in an OLDER turn. Searching the whole request would overwrite
/// that older turn's result — corrupting history — and report no leftover, leaving
/// the CURRENT tool_use unanswered. The results we are folding in belong to the
/// batch being resumed, which is the last one.
///
/// Pure + registry-free so the invariant is directly unit-testable.
fn replace_or_collect_tool_results(
    messages: &mut [ai_providers::ChatMessage],
    fresh: Vec<ai_providers::ContentBlock>,
) -> Vec<ai_providers::ContentBlock> {
    let mut leftovers = Vec::new();

    // Start of the current batch: the last Assistant message carrying a tool_use.
    // Everything from there on is this turn's Assistant/Tool(/User) group.
    //
    // No such message ⇒ there is no batch to fold into, so search NOTHING (every
    // result becomes a leftover). Defaulting to 0 would search the whole request and
    // invert the scope rule above — exactly the cross-turn overwrite this guards.
    let batch_start = messages
        .iter()
        .rposition(|m| {
            matches!(m.role, ai_providers::Role::Assistant)
                && m.content
                    .iter()
                    .any(|b| matches!(b, ai_providers::ContentBlock::ToolUse { .. }))
        })
        .unwrap_or(messages.len());

    for block in fresh {
        let id = match &block {
            ai_providers::ContentBlock::ToolResult { tool_use_id, .. } => tool_use_id.clone(),
            // Not a tool_result (shouldn't happen on this path) — pass through
            // untouched rather than silently dropping model-visible content.
            _ => {
                leftovers.push(block);
                continue;
            }
        };

        let existing = messages[batch_start..].iter_mut().find_map(|m| {
            m.content.iter_mut().find(|b| {
                matches!(b, ai_providers::ContentBlock::ToolResult { tool_use_id, .. } if *tool_use_id == id)
            })
        });

        match existing {
            Some(slot) => {
                tracing::debug!(
                    "replacing an existing tool_result for tool_use_id={} with the \
                     freshly-executed result (was a synthesized placeholder)",
                    id
                );
                *slot = block;
            }
            None => leftovers.push(block),
        }
    }

    leftovers
}

/// ITEM-13/DEC-17: is `(server_id, tool_name)` in an unattended run's allow-list?
/// The allow-list is a JSON array of `{ server_id, tool_name? }` (tool_name
/// absent ⇒ whole server allowed). Parsed generically from `context.metadata`
/// to avoid a type dependency across the extension boundary.
fn unattended_tool_allowed(allow: &serde_json::Value, server_id: &str, tool_name: &str) -> bool {
    allow
        .as_array()
        .map(|arr| {
            arr.iter().any(|g| {
                g.get("server_id").and_then(|v| v.as_str()) == Some(server_id)
                    && g.get("tool_name")
                        .and_then(|v| v.as_str())
                        .map(|t| t == tool_name)
                        .unwrap_or(true)
            })
        })
        .unwrap_or(false)
}

/// Privileged built-in servers (files, memory, elicitation, bio, web_search,
/// lit_search, tool_result). Their tools bypass the MCP approval flow — they're
/// read-only / save-only / user-prompting and auto-attached, so a
/// `read_file`/`remember`/`web_search`/`literature_search`/`get_tool_result`/
/// `ask_user` call must execute immediately rather than stall behind a
/// manual-approval prompt the user never opted into (for `ask_user`, the user
/// answering the form IS the approval).
pub(crate) fn is_builtin_server_id(id: Uuid) -> bool {
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
        // citations is auto-attached for tool-capable chats; writes operate ONLY
        // on the caller's own verified library and never invent data (fabricated
        // DOIs return not_found), so it is approval-bypassed like the others.
        || id == crate::modules::citations::citations_server_id()
        // knowledge_base is approval-bypassed: `search_knowledge` /
        // `list_knowledge_bases` are read-only retrieval over the caller's own
        // KBs; results are treated as untrusted data.
        || id == crate::modules::knowledge_base::knowledge_base_server_id()
        // skill_mcp is approval-bypassed: `load_skill` / `read_skill_file` are
        // read-only reads of skills already installed + available to the caller,
        // auto-attached for tool-capable chats with ≥1 available skill.
        || id == crate::modules::skill_mcp::skill_mcp_server_id()
        // run_js is approval-bypassed for the script START only — the model's
        // `run_js` call auto-runs (like the read-only built-ins), while gated
        // sub-tools called INSIDE the script are individually approved by the
        // js_tool executor. Execution is intercepted inline (see the run_js
        // branch in the execute loop), never dispatched over the loopback.
        || id == crate::modules::js_tool::run_js_mcp_server_id()
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
    /// Per-message map from a BARE tool name (`execute_command`) to the server that
    /// advertised it, populated when the tool list is shipped in `before_llm_call`.
    /// Lets `get_accumulated_content` recover the server_id when a model (e.g.
    /// gpt-oss/harmony) drops the `<server_id>__` prefix ziee prepends. `None`
    /// marks an AMBIGUOUS bare name (≥2 servers) — never auto-resolved.
    /// Key: message_id → (bare_tool_name → Option<server_id>).
    tool_name_server_map: Arc<Mutex<HashMap<Uuid, HashMap<String, Option<Uuid>>>>>,
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
            tool_name_server_map: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Execute a `run_js` tool call inline via the js_tool executor. Gathers the
    /// conversation's accessible tool set (the SAME tools the model sees) into
    /// `ziee.tools.*` host functions, runs the script in the embedded runtime,
    /// and returns the final `McpContentData::ToolResult`. Sub-tool calls
    /// re-enter the dispatcher with `source=Script`; gated ones suspend the
    /// script in-process for per-call approval.
    #[allow(clippy::too_many_arguments)]
    async fn execute_run_js_call(
        &self,
        input: serde_json::Value,
        accessible_servers: &[crate::modules::mcp::models::McpServer],
        tool_use_id: &str,
        context: &StreamContext,
        tx: Option<
            &tokio::sync::mpsc::UnboundedSender<
                Result<axum::response::sse::Event, std::convert::Infallible>,
            >,
        >,
        approval_mode: &crate::modules::mcp::chat_extension::ApprovalMode,
        auto_approved_servers: &[super::approval::models::AutoApprovedServer],
        user_auto_approved: &[super::approval::models::AutoApprovedServer],
    ) -> McpContentData {
        use crate::modules::js_tool::{
            executor, host_bridge::RawTool, limits::JsCaps, run_js_mcp_server_id,
        };

        let run_js_id = run_js_mcp_server_id();

        // Deploy-level kill switch enforced at EXECUTION, not just attachment:
        // the attach flag is already suppressed when disabled, but a
        // hallucinated or history-replayed `run_js` tool_use could still reach
        // here — refuse to execute (audit: perms).
        if !crate::modules::js_tool::is_enabled(&self.config) {
            return McpContentData::ToolResult {
                tool_use_id: tool_use_id.to_string(),
                name: Some("run_js".to_string()),
                server_id: Some(run_js_id.to_string()),
                content: "run_js is disabled on this deployment".to_string(),
                is_error: Some(true),
                attachment: None,
                images: None,
                resource_links: None,
                hidden_content: None,
                structured_content: None,
            };
        }

        let script = input
            .get("script")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        if script.trim().is_empty() {
            return McpContentData::ToolResult {
                tool_use_id: tool_use_id.to_string(),
                name: Some("run_js".to_string()),
                server_id: Some(run_js_id.to_string()),
                content: "run_js error: missing required 'script' argument".to_string(),
                is_error: Some(true),
                attachment: None,
                images: None,
                resource_links: None,
                hidden_content: None,
                structured_content: None,
            };
        }

        // Gather the accessible tool set (excluding run_js itself) as host fns,
        // mirroring the before_llm_call assembly. A server that fails to list is
        // skipped (its tools simply aren't offered to the script).
        let mut tools: Vec<RawTool> = Vec::new();
        let mut auto_approved: std::collections::HashSet<(Uuid, String)> =
            std::collections::HashSet::new();
        for server in accessible_servers.iter().filter(|s| s.id != run_js_id) {
            let session_arc = match self
                .session_manager
                .get_or_create_with_context(
                    server.id,
                    context.user_id,
                    Some(context.conversation_id),
                    Some(context.branch_id),
                    context.message_id,
                    None,
                    McpToolCallSource::Always,
                )
                .await
            {
                Ok(s) => s,
                Err(e) => {
                    tracing::warn!("run_js: skipping server '{}' (session failed): {e}", server.name);
                    continue;
                }
            };
            let listed = {
                let mut session = session_arc.write().await;
                session.list_tools().await
            };
            let mcp_tools = match listed {
                Ok(t) => t,
                Err(e) => {
                    tracing::warn!("run_js: list_tools '{}' failed: {e}", server.name);
                    continue;
                }
            };
            for t in mcp_tools {
                let is_auto = auto_approved_servers
                    .iter()
                    .any(|s| s.server_id == server.id && s.contains_tool(&t.name))
                    || user_auto_approved
                        .iter()
                        .any(|s| s.server_id == server.id && s.contains_tool(&t.name));
                if is_auto {
                    auto_approved.insert((server.id, t.name.clone()));
                }
                tools.push(RawTool {
                    server_id: server.id,
                    server_name: server.name.clone(),
                    tool_name: t.name,
                    description: t.description.unwrap_or_default(),
                    input_schema: t.input_schema,
                });
            }
        }

        // Approvals need the live sse_tx; with no stream, a gated call resolves
        // as "stream closed" → denied (a non-interactive turn can't approve).
        let sse_tx = tx
            .cloned()
            .unwrap_or_else(|| tokio::sync::mpsc::unbounded_channel().0);

        // Read the admin-configurable caps from the DB-backed cache (falling back
        // to defaults if the cache/DB is momentarily unavailable) so an admin
        // change to js_tool_settings applies to the very next run_js invocation.
        let caps = match crate::modules::js_tool::settings_cache::get().await {
            Ok(s) => JsCaps::from_settings(&s),
            Err(_) => JsCaps::default(),
        };

        let run = executor::JsToolRun {
            session_manager: self.session_manager.clone(),
            user_id: context.user_id,
            conversation_id: context.conversation_id,
            branch_id: context.branch_id,
            message_id: context.message_id,
            tool_use_id: tool_use_id.to_string(),
            tools,
            approval_mode: approval_mode.clone(),
            auto_approved,
            sse_tx,
            caps,
        };
        executor::run(run, &script).await
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
        let mut accessible_servers =
            helpers::get_all_accessible_config(&self.pool, context.user_id).await?;
        // Augment with the auto-attached built-in servers (by deterministic id),
        // exactly as the initial classification path does — otherwise an APPROVED
        // tool on a non-group-gated built-in that requires approval (i.e.
        // `control`'s mutating `invoke_capability`) can't be resolved here and
        // fails with "Server not found", so the approved write never executes.
        for id in auto_attach_builtin_ids(&context.metadata) {
            if !accessible_servers.iter().any(|s| s.id == id) {
                if let Some(bs) = crate::core::Repos.mcp.get_any_server(id).await? {
                    if bs.enabled {
                        accessible_servers.push(bs);
                    }
                }
            }
        }

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

            // CLAIM the approval BEFORE executing. The row is what makes a tool
            // eligible to run, so consuming it first is what makes execution
            // exactly-once: `get_approved_tools_for_branch` can no longer hand it to
            // a subsequent pass. This used to run AFTER execution with its error
            // swallowed, so a failed DELETE silently re-ran the tool and appended a
            // SECOND tool_result row for this tool_use_id.
            //
            // The DELETE is the claim, and its ROW COUNT is the verdict — that is the
            // whole point, so all three outcomes are handled distinctly:
            //   Ok(true)  — we deleted the row: we own this execution.
            //   Ok(false) — zero rows: a concurrent pass already claimed it and is
            //               executing it. We must NOT run it again.
            //   Err       — the DB is unhealthy; we cannot tell whether the row is
            //               gone. Do not execute: a double-run of a side-effecting
            //               tool is the worse outcome.
            //
            // A non-Won outcome still PUSHES an is_error result and continues — it
            // must, and this is load-bearing. Every sibling arm in this loop does the
            // same, because a tool_use with no result is unrecoverable: for a batch
            // where nothing ran, `group_assistant_blocks`' `batch_has_result` gate
            // emits a BARE Assistant turn with no Tool message, so that tool_use stays
            // unpaired on EVERY subsequent request and the branch is bricked. (The
            // bare turn is correct only for the awaiting-approval case, whose result
            // is still coming.) Skipping silently, or bailing with `return Err`, would
            // also discard the real results of approvals earlier in this same batch
            // that already won their claims and executed — their side effects done,
            // their rows gone, their results never persisted. Trading a rare
            // duplicate for a permanently unusable conversation is a bad trade.
            //
            // So: the error result keeps the request VALID. Exactly-once execution
            // still holds where it matters — Ok(true) is the only branch that runs the
            // tool.
            //
            // Known cost, accepted deliberately: on AlreadyClaimed this pass persists
            // its error result while the winner is still executing, so the error takes
            // the LOWER sequence_order and assembly's keep-first makes it
            // authoritative — the model is told the tool was not run here even though
            // the winner ran it, and the winner's output is not what it reads. That is
            // the right trade: the alternative (emit nothing) leaves the tool_use
            // unpaired, which is not a worse ANSWER but a dead conversation. The copy
            // therefore tells the model to ask the user to retry rather than implying
            // the tool never ran anywhere. Reaching this at all needs two concurrent
            // passes over one branch.
            let claim = Repos
                .chat
                .mcp
                .delete_tool_approval(tool_use_id.clone(), approval.message_id)
                .await;
            let outcome = claim_outcome(claim.as_ref().map(|won| *won));
            if outcome != ClaimOutcome::Won {
                let reason = match &outcome {
                    ClaimOutcome::AlreadyClaimed => {
                        tracing::warn!(
                            "Approval for tool_use_id={} was already claimed by another pass; \
                             not executing it again.",
                            tool_use_id
                        );
                        "it was already started by another request"
                    }
                    _ => {
                        let e = claim.as_ref().err().expect("non-Won, non-AlreadyClaimed is Err");
                        tracing::error!(
                            "Failed to claim the approval record for tool_use_id={}: {}. Not \
                             executing, to avoid running the tool twice.",
                            tool_use_id,
                            e
                        );
                        "its approval could not be confirmed"
                    }
                };
                tool_results.push(
                    McpContentData::ToolResult {
                        tool_use_id: tool_use_id.clone(),
                        name: Some(tool_name.clone()),
                        server_id: approval.server_id.map(|id| id.to_string()),
                        content: format!(
                            "The tool '{tool_name}' was not run here because {reason}. It was \
                             not executed twice. Ask the user to retry if you still need it."
                        ),
                        is_error: Some(true),
                        attachment: None,
                        images: None,
                        resource_links: None,
                        hidden_content: None,
                        structured_content: None,
                    }
                    .to_message_content(),
                );
                executed_tool_use_ids.push(tool_use_id.clone());
                continue;
            }

            // Use server_id from approval record (stored separately)
            let server_id = match approval.server_id {
                Some(id) => id,
                None => {
                    // The tool_use never resolved to a server (e.g. the model
                    // returned a bare tool name with no `<server_id>__` prefix and
                    // it could not be matched to an advertised tool). Surface a
                    // clear error. The approval row is already gone — the claim at
                    // the top of this iteration deleted it — so the loop still can't
                    // spin here to `max_iteration` (the reported bug).
                    tracing::error!("No server_id in approval record for tool: {}", tool_name);
                    let error_result = McpContentData::ToolResult {
                        tool_use_id: tool_use_id.clone(),
                        name: Some(tool_name.clone()),
                        server_id: None,
                        content: format!(
                            "Could not resolve an MCP server for tool '{}'. The model returned \
                             a tool name without a server prefix and it could not be matched to \
                             an advertised tool, so the call was not executed. Retry, or select \
                             the tool explicitly.",
                            tool_name
                        ),
                        is_error: Some(true),
                        attachment: None,
                        images: None,
                        resource_links: None,
                        hidden_content: None,
                        structured_content: None,
                    };
                    tool_results.push(error_result.to_message_content());
                    executed_tool_use_ids.push(tool_use_id.clone());
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
                executed_tool_use_ids.push(tool_use_id.clone());
                // (Approval row already claimed at the top of this iteration, so this
                // branch still can't re-loop to max_iteration.)
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
                            // Build from the UN-REDACTED server row: the accessible
                            // list nulls is_system URLs, which would fail
                            // new_with_sampling with MISSING_URL.
                            let built = match self
                                .session_manager
                                .resolve_server_for_session(server.id)
                                .await
                            {
                                Ok(real_server) => {
                                    McpSession::new_with_sampling(real_server, handler).await
                                }
                                Err(e) => Err(e),
                            };
                            match built {
                                Ok(mut s) => {
                                    s.set_call_context(McpCallContext {
                                        user_id: Some(context.user_id),
                                        conversation_id: Some(context.conversation_id),
                                        branch_id: Some(context.branch_id),
                                        message_id: context.message_id,
                                        tool_use_id: Some(tool_use_id.clone()),
                                        source: McpToolCallSource::Sampling,
                                        server_name: server.name.clone(),
                                        is_built_in: server.is_built_in,
                                        ..Default::default()
                                    });
                                    _owned = Some(s);
                                }
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
                    executed_tool_use_ids.push(tool_use_id.clone());
                    // (Approval row already claimed at the top of this iteration, so this
                    // branch still can't re-loop to max_iteration.)
                    continue;
                }
                let arc = match self.session_manager
                    .get_or_create_with_context(
                        server.id,
                        context.user_id,
                        Some(context.conversation_id),
                        Some(context.branch_id),
                        context.message_id,
                        Some(tool_use_id.clone()),
                        McpToolCallSource::Approval,
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
                        executed_tool_use_ids.push(tool_use_id.clone());
                        // (Approval row already claimed at the top of this iteration, so this
                        // branch still can't re-loop to max_iteration.)
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

            // Persist any resource_links the tool returned into durable file-store
            // artifacts via the shared consumer. It handles every URI shape uniformly:
            // is_saved links are referenced (not re-saved), `ziee://<host_path>` links
            // from trusted in-process tools are read off disk behind path-confinement
            // guards, and external / loopback links are fetched over HTTP. It stamps
            // file_id/version onto each saved link and strips raw host paths before it
            // returns. saved_artifacts: (artifact_id, display_name, download_url);
            // saved_file_urls: (display_name, download_url) for is_saved links.
            let mut saved_artifacts: Vec<(Uuid, String, Option<String>)> = Vec::new();
            let mut saved_file_urls: Vec<(String, String)> = Vec::new();
            if let McpContentData::ToolResult { resource_links: Some(ref mut links), is_error, .. } = result
                && !is_error.unwrap_or(false)
            {
                // `ziee://` reads are confined to this conversation's sandbox workspace
                // (code_sandbox is the only is_saved:false producer today). Empty when the
                // sandbox is uninitialized → a stray ziee:// link simply fails confinement.
                let allowed_roots: Vec<std::path::PathBuf> =
                    crate::modules::code_sandbox::config::get_state()
                        .map(|s| vec![s.workspace_root.join(context.conversation_id.to_string())])
                        .unwrap_or_default();

                // Same-host trust set for re-hosting this external server's result files (see
                // `resource_link::result_link_trusted_hosts`): the hosts of the user's accessible,
                // enabled, NON-built-in MCP servers — incl. admin-registered system servers with a
                // real external `url` (e.g. `host.docker.internal`) whose url is redacted in the
                // user-facing list. A built-in emitter short-circuits to empty (its links are trusted
                // loopback URLs the trust set is never consulted for).
                let trusted_hosts = crate::modules::mcp::resource_link::result_link_trusted_hosts(
                    server.is_built_in,
                    context.user_id,
                )
                .await;

                let outcome = crate::modules::mcp::resource_link::persist_links(
                    links,
                    context.user_id,
                    Some(context.conversation_id),
                    context.message_id,
                    "mcp",
                    None, // workflow_run_id: chat path, not a workflow run
                    server.id,
                    server.is_built_in,
                    &server.headers,
                    &trusted_hosts,
                    &allowed_roots,
                    Some(self.config.jwt.secret.as_str()),
                    Some(self.config.jwt.issuer.as_str()),
                    Some(self.config.jwt.audience.as_str()),
                )
                .await
                .unwrap_or_default();

                // is_saved:true links pass straight through to the hidden-content list.
                saved_file_urls = outcome.referenced;

                // For each newly-ingested artifact: emit the per-artifact SSE event and
                // mint a token-signed download URL the LLM can hand to another tool.
                for art in &outcome.saved {
                    helpers::send_artifact_created_event(
                        tx,
                        &tool_use_id,
                        &art.file_id.to_string(),
                        &art.filename,
                        art.mime_type.as_deref(),
                        art.size,
                    )
                    .await;

                    let download_url = {
                        use crate::modules::file::types::{DownloadTokenClaims, DOWNLOAD_TOKEN_AUDIENCE};
                        use jsonwebtoken::{encode, EncodingKey, Header as JwtHeader};
                        let now = chrono::Utc::now().timestamp() as usize;
                        let claims = DownloadTokenClaims {
                            file_id: art.file_id.to_string(),
                            user_id: context.user_id.to_string(),
                            version: None,
                            exp: now + 3600,
                            iat: now,
                            iss: self.config.jwt.issuer.clone(),
                            aud: DOWNLOAD_TOKEN_AUDIENCE.to_string(),
                        };
                        // Root the tool-to-tool download URL at the SAME origin
                        // get_resource_link uses (public_base_url when set, else the pinned
                        // 127.0.0.1 loopback) — NOT self.config.server.host, which may be a
                        // bind address unreachable by the MCP server the LLM passes it to.
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
                        .map(|token| {
                            build_artifact_download_url(
                                &origin,
                                &self.config.server.api_prefix,
                                art.file_id,
                                &token,
                            )
                        })
                    };
                    saved_artifacts.push((art.file_id, art.filename.clone(), download_url));
                }
            }

            // Update tool result content with the saved artifact info so the LLM knows the
            // file_ids. Also set hidden_content with token-based download URLs — included in
            // LLM messages but stripped from browser API responses. (file_id/version are
            // already stamped onto each resource_link by persist_links above.)
            if (!saved_artifacts.is_empty() || !saved_file_urls.is_empty())
                && let McpContentData::ToolResult { ref mut content, ref mut hidden_content, .. } = result {
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
                        *hidden_content =
                            Some(saved_artifact_hidden_content_guidance(&url_lines.join("\n")));
                    }
                }

            // Track executed tool_use_id
            executed_tool_use_ids.push(tool_use_id.clone());

            // (The approval record was already claimed/deleted at the top of this
            // loop iteration, before execution — see the claim comment there.)

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
        // ITEM-13/DEC-17: stash the unattended signal + allow-list into context
        // metadata so `after_llm_call` (which has no `send_request`) can branch
        // the approval decision to deny-not-pause. Default-false, so an
        // interactive request is byte-identical (nothing is inserted).
        if send_request.unattended {
            context
                .metadata
                .insert("unattended".to_string(), serde_json::json!(true));
            context.metadata.insert(
                "unattended_allowed_tools".to_string(),
                serde_json::to_value(&send_request.unattended_allowed_tools)
                    .unwrap_or_else(|_| serde_json::json!([])),
            );
        }

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

            // Fold the results (approved + denied) into the request. Any id that
            // already carries a tool_result — a placeholder `group_assistant_blocks`
            // synthesized for a tool that was still awaiting approval when the batch
            // was last assembled — is REPLACED in place; only genuinely-new ids are
            // appended as a single user message. Blindly pushing them all here is
            // what produced two tool_result blocks for one tool_use_id and the
            // provider's "each tool_use must have a single result" rejection.
            let content_blocks =
                replace_or_collect_tool_results(&mut request.messages, content_blocks);
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

        // Validate and build server configuration. `accessible_servers` is
        // reused below instead of re-fetching the same accessible-server list.
        let (mut server_configs, accessible_ids, mut accessible_servers) =
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

        // Reuse the accessible-server list already fetched by
        // `validate_and_build_config` (+ the auto-attached built-ins so the
        // tool-listing loop can resolve their details).
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

                    // Create session (with sampling if supported). Build from the
                    // UN-REDACTED server row: the accessible list nulls is_system
                    // URLs, which would fail every build below with MISSING_URL.
                    let session_result = match self
                        .session_manager
                        .resolve_server_for_session(server.id)
                        .await
                    {
                        Err(e) => Err(e),
                        Ok(real_server) => {
                            if server.supports_sampling {
                                if let Some(model_id) = maybe_model_id {
                                    match ChatSamplingHandler::new(model_id, context.user_id).await {
                                        Ok(h) => McpSession::new_with_sampling(real_server, Arc::new(h)).await,
                                        Err(e) => {
                                            tracing::warn!("Always-mode: failed to init sampling provider for {}: {}", server.name, e);
                                            McpSession::new(real_server).await
                                        }
                                    }
                                } else {
                                    McpSession::new(real_server).await
                                }
                            } else {
                                McpSession::new(real_server).await
                            }
                        }
                    };

                    match session_result {
                        Err(e) => {
                            tracing::warn!("Always-mode: failed to connect to server {}: {}", server.name, e);
                        }
                        Ok(mut session) => {
                            // Record always-mode pre-runs (the session is built
                            // directly, bypassing the manager's stamping).
                            session.set_call_context(McpCallContext {
                                user_id: Some(context.user_id),
                                conversation_id: Some(context.conversation_id),
                                branch_id: Some(context.branch_id),
                                message_id: context.message_id,
                                source: McpToolCallSource::Always,
                                server_name: server.name.clone(),
                                is_built_in: server.is_built_in,
                                ..Default::default()
                            });
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
                    Some(context.branch_id),
                    context.message_id,
                    // Tool-collection session (list_tools only); source/tool_use moot.
                    None,
                    McpToolCallSource::Always,
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

        // Stash a per-message `bare_tool_name -> Option<server_id>` map from the
        // tools we actually advertised this turn, so `get_accumulated_content` can
        // recover the server when a model (e.g. gpt-oss/harmony) returns a tool
        // call WITHOUT the `<server_id>__` prefix. A bare name advertised by ≥2
        // servers is marked `None` (ambiguous) and never auto-resolved. Built from
        // `all_tools` (the exact composed names shipped) so it matches what the
        // model saw and never resolves a tool that was dropped from the list.
        if let Some(message_id) = context.message_id {
            let mut bare_map: HashMap<String, Option<Uuid>> = HashMap::new();
            for tool in &all_tools {
                let composed = &tool.function.name;
                if let Some((id_str, bare)) = composed.split_once("__")
                    && let Ok(sid) = Uuid::parse_str(id_str)
                {
                    match bare_map.get(bare) {
                        // Same bare name from a different server → ambiguous.
                        Some(Some(existing)) if *existing != sid => {
                            bare_map.insert(bare.to_string(), None);
                        }
                        Some(_) => { /* already Some(same) or already None (ambiguous) */ }
                        None => {
                            bare_map.insert(bare.to_string(), Some(sid));
                        }
                    }
                }
            }
            if let Ok(mut guard) = self.tool_name_server_map.lock() {
                // Normally the matching `get_accumulated_content` removes this
                // entry at finalize; a stream that errors/aborts before finalize
                // would orphan it. Bound the map so those leaks can't grow without
                // limit — it's a best-effort per-turn recovery cache, so clearing
                // stale entries at most degrades a concurrent bare-name call to the
                // clear "could not resolve" error, which self-heals next turn.
                const MAX_PENDING_TOOL_MAPS: usize = 1024;
                if guard.len() >= MAX_PENDING_TOOL_MAPS && !guard.contains_key(&message_id) {
                    tracing::warn!(
                        "tool_name_server_map exceeded {} entries; clearing stale recovery cache \
                         (streams that aborted before finalize)",
                        MAX_PENDING_TOOL_MAPS
                    );
                    guard.clear();
                }
                guard.insert(message_id, bare_map);
            }
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

        // Fetch this conversation's MCP settings ONCE for the whole call; both
        // the loop-settings check (STEP 0) and the approval check below derive
        // from it (previously two separate get_conversation_settings round-trips
        // per after_llm_call iteration).
        let conv_settings = crate::core::Repos
            .chat
            .mcp
            .get_conversation_settings(context.conversation_id)
            .await?;

        // === STEP 0: Check loop settings ===
        // Get loop settings from conversation MCP settings (or use defaults)
        let loop_settings = conv_settings
            .as_ref()
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

        // Check MCP approval settings for this conversation (reuses the single
        // fetch from the top of after_llm_call).
        let settings = conv_settings;

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
        // Control is auto-attached but NOT on `is_builtin_server_id` (its writes
        // require approval). It must still count here so a Disabled-approval
        // conversation does NOT early-return on a control-only turn — otherwise
        // the control `tool_use` would be left without a paired `tool_result`.
        // Reaching the classification loop, a control call in Disabled mode takes
        // the `tools_disabled` path (a synthesized denial), which is correct: MCP
        // is off, so control doesn't run, but the tool_use is properly answered.
        let has_builtin_call = tool_uses.iter().any(|(_, _, sid, _)| {
            uuid::Uuid::parse_str(sid)
                .map(|id| {
                    is_builtin_server_id(id)
                        || id == crate::modules::control_mcp::control_mcp_server_id()
                })
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

        // ITEM-13: unattended (scheduled) run signals stashed by before_llm_call.
        // A tool that would need approval and is NOT allow-listed is denied here
        // (turn continues) rather than creating an orphaned pending approval.
        let unattended = context
            .metadata
            .get("unattended")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let unattended_allowed = context
            .metadata
            .get("unattended_allowed_tools")
            .cloned()
            .unwrap_or_else(|| serde_json::json!([]));
        // (tool_use_id, tool_name, server_id) denied because unattended + not allow-listed.
        let mut tools_denied_unattended: Vec<(String, String, String)> = Vec::new();

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

            // The control server is auto-attached but NOT approval-bypassed:
            // read-only control tools auto-run, but a mutating `invoke_capability`
            // ALWAYS requires explicit approval — overriding even AutoApprove.
            let is_control = uuid::Uuid::parse_str(&server_id)
                .map(|id| id == crate::modules::control_mcp::control_mcp_server_id())
                .unwrap_or(false);

            let needs_approval = if is_control {
                crate::modules::control_mcp::handlers::control_call_needs_approval(
                    &tool_name, &input,
                )
            } else if is_builtin {
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
                // ITEM-13/DEC-17: in an unattended run, an approval-required tool is
                // resolved by the task's allow-list, NOT by a live approval prompt
                // (there is no user to answer). An ALLOW-LISTED tool was pre-
                // authorized by the task creator → it AUTO-RUNS (that is the whole
                // point of the allow-list). A NON-allow-listed tool is DENIED (a
                // synthesized denial tool_result; no orphaned pending row, no
                // truncation) so the turn continues. (Blind-audit fix: an allow-
                // listed tool previously fell through to a pause that no one could
                // resolve + was omitted from the skipped report.)
                if unattended {
                    if unattended_tool_allowed(&unattended_allowed, &server_id, &tool_name) {
                        tools_to_execute.push((tool_use_id, tool_name, server_id, input));
                    } else {
                        tools_denied_unattended.push((tool_use_id, tool_name, server_id));
                    }
                    continue;
                }
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

            // Resolve (server_id, server_name) for each tool, then insert all
            // pending-approval rows in ONE round-trip (was an N+1 INSERT loop).
            let resolved: Vec<(Option<uuid::Uuid>, String)> = tools_needing_approval
                .iter()
                .map(|(_, _, server_id_str, _)| {
                    if let Ok(id) = uuid::Uuid::parse_str(server_id_str) {
                        let name = accessible_servers
                            .iter()
                            .find(|s| s.id == id)
                            .map(|s| s.name.clone())
                            .unwrap_or_else(|| id.to_string());
                        (Some(id), name)
                    } else {
                        (None, server_id_str.to_string())
                    }
                })
                .collect();

            let new_approvals: Vec<crate::modules::mcp::chat_extension::approval::repository::NewToolApproval> =
                tools_needing_approval
                    .iter()
                    .zip(resolved.iter())
                    .map(|((tool_use_id, tool_name, _, input), (server_id, server_name))| {
                        crate::modules::mcp::chat_extension::approval::repository::NewToolApproval {
                            tool_use_id: tool_use_id.clone(),
                            tool_name: tool_name.clone(),
                            tool_input: input.clone(),
                            server_id: *server_id,
                            server_name: server_name.clone(),
                        }
                    })
                    .collect();

            let created = crate::core::Repos
                .chat
                .mcp
                .create_tool_approvals(
                    context.conversation_id,
                    context.branch_id,
                    final_message.id,
                    context.user_id,
                    &new_approvals,
                )
                .await?;
            tracing::info!(
                "Created {} pending approval records for branch_id={}",
                created.len(), context.branch_id
            );

            // Fan out the per-tool SSE events (keyed off the input list, not the
            // RETURNING order which is not guaranteed to match).
            for ((tool_use_id, tool_name, server_id_str, input), (_, server_name)) in
                tools_needing_approval.iter().zip(resolved.iter())
            {
                helpers::send_approval_required_event(tx, tool_use_id, tool_name, server_name, server_id_str, input).await?;
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

        // ITEM-13/17: unattended-denied tools get a denial tool_result too (turn
        // stays protocol-valid + continues), with a structured marker the
        // scheduler reads back for its skipped-tools report. Because these are
        // NOT in `tools_needing_approval`, the pause-for-approval block below is
        // skipped → no orphaned pending rows, no truncation.
        for (tool_use_id, tool_name, server_id_str) in &tools_denied_unattended {
            let denial = McpContentData::ToolResult {
                tool_use_id: tool_use_id.clone(),
                name: Some(tool_name.clone()),
                server_id: Some(server_id_str.clone()),
                content: format!(
                    "Tool '{tool_name}' requires approval and is not permitted to run \
                     unattended for this scheduled task; it was skipped."
                ),
                is_error: Some(true),
                attachment: None,
                images: None,
                resource_links: None,
                hidden_content: None,
                structured_content: Some(serde_json::json!({
                    "unattended_denied": true,
                    "tool_name": tool_name,
                })),
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
                    // Emit an error tool_result so the model's tool_use block
                    // is not orphaned (a tool_use with no matching tool_result
                    // breaks the next provider request). Mirrors the
                    // server-not-found branch below.
                    let error_result = McpContentData::ToolResult {
                        tool_use_id: tool_use_id.clone(),
                        name: Some(tool_name.clone()),
                        server_id: Some(server_id_str.clone()),
                        content: format!("Invalid server id '{}'", server_id_str),
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
                == crate::modules::js_tool::run_js_mcp_server_id()
            {
                // `run_js` is executed INLINE — it needs the live chat context
                // (session manager, the accessible tool set, sse_tx, and the
                // approval channel), so intercept here before any loopback
                // dispatch, exactly like `ask_user` below. `false` = the model
                // still reasons about the returned final value.
                (
                    self.execute_run_js_call(
                        input,
                        &accessible_servers,
                        &tool_use_id,
                        context,
                        tx,
                        &approval_mode,
                        &auto_approved_servers,
                        &user_auto_approved,
                    )
                    .await,
                    false,
                )
            } else if server.id
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
                                    // Build from the UN-REDACTED server row: the
                                    // accessible list nulls is_system URLs, which
                                    // would fail new_with_sampling with MISSING_URL.
                                    let built = match self
                                        .session_manager
                                        .resolve_server_for_session(server.id)
                                        .await
                                    {
                                        Ok(real_server) => {
                                            McpSession::new_with_sampling(real_server, Arc::new(h)).await
                                        }
                                        Err(e) => Err(e),
                                    };
                                    match built {
                                        Ok(mut sampling_session) => {
                                            sampling_session.set_call_context(McpCallContext {
                                                user_id: Some(context.user_id),
                                                conversation_id: Some(context.conversation_id),
                                                branch_id: Some(context.branch_id),
                                                message_id: context.message_id,
                                                tool_use_id: Some(tool_use_id.clone()),
                                                source: McpToolCallSource::Sampling,
                                                server_name: server.name.clone(),
                                                is_built_in: server.is_built_in,
                                                ..Default::default()
                                            });
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
                                            // Log the full error (may contain the upstream URL) server-side only.
                                            // The user-facing content names the server, NOT the raw error, so an
                                            // is_system server's redacted admin URL is never disclosed to the user.
                                            tracing::error!("Failed to create sampling session for {}: {}", server.name, e);
                                            (McpContentData::ToolResult {
                                                tool_use_id: tool_use_id.clone(),
                                                name: Some(tool_name.clone()),
                                                server_id: Some(server.id.to_string()),
                                                content: format!("Failed to connect to server '{}'", server.name),
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
                        Some(context.branch_id),
                        context.message_id,
                        Some(tool_use_id.clone()),
                        McpToolCallSource::Chat,
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

            // Persist any resource_links the tool returned into durable file-store
            // artifacts via the shared consumer. It handles every URI shape uniformly:
            // is_saved links are referenced (not re-saved), `ziee://<host_path>` links
            // from trusted in-process tools are read off disk behind path-confinement
            // guards, and external / loopback links are fetched over HTTP. It stamps
            // file_id/version onto each saved link and strips raw host paths before it
            // returns. saved_artifacts: (artifact_id, display_name, download_url);
            // saved_file_urls: (display_name, download_url) for is_saved links.
            let mut saved_artifacts: Vec<(Uuid, String, Option<String>)> = Vec::new();
            let mut saved_file_urls: Vec<(String, String)> = Vec::new();
            if let McpContentData::ToolResult { resource_links: Some(ref mut links), is_error, .. } = result
                && !is_error.unwrap_or(false)
            {
                // `ziee://` reads are confined to this conversation's sandbox workspace
                // (code_sandbox is the only is_saved:false producer today). Empty when the
                // sandbox is uninitialized → a stray ziee:// link simply fails confinement.
                let allowed_roots: Vec<std::path::PathBuf> =
                    crate::modules::code_sandbox::config::get_state()
                        .map(|s| vec![s.workspace_root.join(context.conversation_id.to_string())])
                        .unwrap_or_default();

                // Same-host trust set for re-hosting this external server's result files (see
                // `resource_link::result_link_trusted_hosts`): the hosts of the user's accessible,
                // enabled, NON-built-in MCP servers — incl. admin-registered system servers with a
                // real external `url` (e.g. `host.docker.internal`) whose url is redacted in the
                // user-facing list. A built-in emitter short-circuits to empty (its links are trusted
                // loopback URLs the trust set is never consulted for).
                let trusted_hosts = crate::modules::mcp::resource_link::result_link_trusted_hosts(
                    server.is_built_in,
                    context.user_id,
                )
                .await;

                let outcome = crate::modules::mcp::resource_link::persist_links(
                    links,
                    context.user_id,
                    Some(context.conversation_id),
                    context.message_id,
                    "mcp",
                    None, // workflow_run_id: chat path, not a workflow run
                    server.id,
                    server.is_built_in,
                    &server.headers,
                    &trusted_hosts,
                    &allowed_roots,
                    Some(self.config.jwt.secret.as_str()),
                    Some(self.config.jwt.issuer.as_str()),
                    Some(self.config.jwt.audience.as_str()),
                )
                .await
                .unwrap_or_default();

                // is_saved:true links pass straight through to the hidden-content list.
                saved_file_urls = outcome.referenced;

                // For each newly-ingested artifact: emit the per-artifact SSE event and
                // mint a token-signed download URL the LLM can hand to another tool.
                for art in &outcome.saved {
                    helpers::send_artifact_created_event(
                        tx,
                        &tool_use_id,
                        &art.file_id.to_string(),
                        &art.filename,
                        art.mime_type.as_deref(),
                        art.size,
                    )
                    .await;

                    let download_url = {
                        use crate::modules::file::types::{DownloadTokenClaims, DOWNLOAD_TOKEN_AUDIENCE};
                        use jsonwebtoken::{encode, EncodingKey, Header as JwtHeader};
                        let now = chrono::Utc::now().timestamp() as usize;
                        let claims = DownloadTokenClaims {
                            file_id: art.file_id.to_string(),
                            user_id: context.user_id.to_string(),
                            version: None,
                            exp: now + 3600,
                            iat: now,
                            iss: self.config.jwt.issuer.clone(),
                            aud: DOWNLOAD_TOKEN_AUDIENCE.to_string(),
                        };
                        // Root the tool-to-tool download URL at the SAME origin
                        // get_resource_link uses (public_base_url when set, else the pinned
                        // 127.0.0.1 loopback) — NOT self.config.server.host, which may be a
                        // bind address unreachable by the MCP server the LLM passes it to.
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
                        .map(|token| {
                            build_artifact_download_url(
                                &origin,
                                &self.config.server.api_prefix,
                                art.file_id,
                                &token,
                            )
                        })
                    };
                    saved_artifacts.push((art.file_id, art.filename.clone(), download_url));
                }
            }

            // Update tool result content with the saved artifact info so the LLM knows the
            // file_ids. Also set hidden_content with token-based download URLs — included in
            // LLM messages but stripped from browser API responses. (file_id/version are
            // already stamped onto each resource_link by persist_links above.)
            if (!saved_artifacts.is_empty() || !saved_file_urls.is_empty())
                && let McpContentData::ToolResult { ref mut content, ref mut hidden_content, .. } = result {
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
                        *hidden_content =
                            Some(saved_artifact_hidden_content_guidance(&url_lines.join("\n")));
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

        // Drain this message's accumulated entries (sorted by index for
        // deterministic id assignment), then drop the accumulator lock BEFORE any
        // `.await` — never hold a std Mutex across await.
        let mut drained: Vec<(usize, AccumulatedToolUse)> = {
            let mut accumulator = self
                .tool_use_accumulator
                .lock()
                .map_err(|e| AppError::internal_error(format!("Failed to lock accumulator: {}", e)))?;
            let keys: Vec<(Uuid, usize)> = accumulator
                .keys()
                .filter(|(msg_id, _)| *msg_id == message_id)
                .copied()
                .collect();
            keys.into_iter()
                .filter_map(|key| accumulator.remove(&key).map(|acc| (key.1, acc)))
                .collect()
        };
        drained.sort_by_key(|(index, _)| *index);

        // Snapshot + clear the per-message bare-name→server_id recovery map that
        // `before_llm_call` populated for this turn (symmetric with the accumulator
        // drain above). Used to recover server_id when the model dropped the prefix.
        let bare_name_map: HashMap<String, Option<Uuid>> = self
            .tool_name_server_map
            .lock()
            .ok()
            .and_then(|mut g| g.remove(&message_id))
            .unwrap_or_default();

        // Seed the used-id set from tool_use ids already persisted on this message
        // (prior loop iterations) so a fresh call with the same provider id gets a
        // distinct id (cross-iteration collision). Targeted query on the indexed
        // `content_type` column so we only load the small tool_use rows — never the
        // (potentially large, per-iteration-accumulating) tool_result blobs.
        // Degrade to empty on DB error.
        let mut used_ids: std::collections::HashSet<String> = std::collections::HashSet::new();
        match sqlx::query_scalar::<_, serde_json::Value>(
            "SELECT content FROM message_contents \
             WHERE message_id = $1 AND content_type = 'tool_use'",
        )
        .bind(message_id)
        .fetch_all(&self.pool)
        .await
        {
            Ok(rows) => {
                for raw in rows {
                    if let Ok(data) = serde_json::from_value::<MessageContentData>(raw)
                        && let Ok(McpContentData::ToolUse { id, .. }) =
                            McpContentData::from_message_content(&data)
                        && !id.is_empty()
                    {
                        used_ids.insert(id);
                    }
                }
            }
            Err(e) => {
                tracing::warn!(
                    "get_accumulated_content: could not load existing tool_use ids for message {} \
                     ({}); proceeding with within-batch dedup only",
                    message_id,
                    e
                );
            }
        }

        let mut content_blocks = Vec::new();

        // Convert each accumulated tool use
        for (index, accumulated) in drained {
            // Parse accumulated JSON input
            let input = serde_json::from_str(&accumulated.input_json).unwrap_or_else(|e| {
                tracing::error!(
                    "Failed to parse accumulated tool use input JSON: {}. Input: {}",
                    e,
                    accumulated.input_json
                );
                serde_json::json!({}) // Fallback to empty object
            });

            // Resolve (server_id, tool_name) from the accumulated wire name — a
            // well-formed `<uuid>__tool`, or a prefix-less name recovered from the
            // tools advertised this turn (see `resolve_server_and_tool`).
            let full_name = accumulated.name.unwrap_or_default();
            let was_well_formed = full_name
                .split_once("__")
                .is_some_and(|(id, _)| Uuid::parse_str(id).is_ok());
            let (recovered_sid, tool_name) =
                resolve_server_and_tool(&full_name, &bare_name_map);
            let server_id = match recovered_sid {
                Some(sid) => {
                    if !was_well_formed {
                        tracing::info!(
                            "[mcp] Recovered server_id for prefix-less tool name '{}' -> '{}': {}",
                            full_name,
                            tool_name,
                            sid
                        );
                    }
                    sid.to_string()
                }
                None => {
                    tracing::warn!(
                        "[mcp] Tool name has no valid server_id prefix and is not uniquely \
                         recoverable: {}",
                        full_name
                    );
                    String::new()
                }
            };

            // Ensure the tool_use id is non-empty and unique within this message,
            // even when the provider streams an empty or duplicate id. Track
            // assigned ids so two calls in one batch also stay distinct.
            let provider_id = accumulated.id.unwrap_or_default();
            let tool_use_id = resolve_unique_tool_use_id(&provider_id, &used_ids);
            used_ids.insert(tool_use_id.clone());

            tracing::info!(
                "MCP: Finalized tool use at index {}: id={}, name={}, server_id={}",
                index,
                tool_use_id,
                tool_name,
                server_id,
            );

            // Create McpContentData::ToolUse with separate server_id and name
            let tool_use = McpContentData::ToolUse {
                id: tool_use_id,
                name: tool_name.clone(),
                server_id: server_id.clone(),
                input,
            };

            content_blocks.push((index, tool_use.to_message_content()));
        }

        Ok(content_blocks)
    }
}

#[cfg(test)]
mod tests {
    use super::{
        build_artifact_download_url, claim_outcome, file_download_origin,
        replace_or_collect_tool_results, saved_artifact_hidden_content_guidance,
        tool_system_guidance, ClaimOutcome,
    };
    use crate::core::config::CodeSandboxConfig;
    use uuid::Uuid;

    fn tool(name: &str) -> ai_providers::Tool {
        ai_providers::Tool::function(name.to_string(), String::new(), serde_json::json!({}))
    }

    // ── fix-duplicate-tool-result ────────────────────────────────────────────
    // Fixtures mirror the ones in chat/core/services/streaming.rs's `mod tests`
    // (same wire types, same shapes) so the two suites read alike.

    fn tool_use(id: &str, name: &str) -> ai_providers::ContentBlock {
        ai_providers::ContentBlock::ToolUse {
            id: id.to_string(),
            name: name.to_string(),
            input: serde_json::json!({}),
        }
    }

    fn tool_result(id: &str, content: &str) -> ai_providers::ContentBlock {
        ai_providers::ContentBlock::ToolResult {
            tool_use_id: id.to_string(),
            name: None,
            content: vec![ai_providers::ContentBlock::Text {
                text: content.to_string(),
            }],
            is_error: None,
        }
    }

    /// Every `tool_use_id` answered by a `tool_result`, across the WHOLE request.
    fn result_ids_all(msgs: &[ai_providers::ChatMessage]) -> Vec<String> {
        msgs.iter()
            .flat_map(|m| m.content.iter())
            .filter_map(|b| match b {
                ai_providers::ContentBlock::ToolResult { tool_use_id, .. } => {
                    Some(tool_use_id.clone())
                }
                _ => None,
            })
            .collect()
    }

    fn result_text(b: &ai_providers::ContentBlock) -> String {
        match b {
            ai_providers::ContentBlock::ToolResult { content, .. } => match &content[0] {
                ai_providers::ContentBlock::Text { text } => text.clone(),
                _ => panic!("expected a text block inside the tool_result"),
            },
            _ => panic!("expected a tool_result"),
        }
    }

    /// The provider invariant this feature exists to hold: no `tool_use_id` is
    /// answered by more than one `tool_result` anywhere in the request.
    fn assert_single_result_per_tool_use(msgs: &[ai_providers::ChatMessage]) {
        let ids = result_ids_all(msgs);
        let mut seen = std::collections::HashSet::new();
        for id in &ids {
            assert!(
                seen.insert(id.clone()),
                "tool_use_id {id} is answered by MORE THAN ONE tool_result — the provider \
                 rejects this: \"each tool_use must have a single result\". All results: {ids:?}"
            );
        }
    }

    /// TEST-1 — THE REPRO, driven through the real production functions.
    ///
    /// A mixed parallel batch: built-in A (approval-exempt) + external B (needs
    /// approval). At the pause only A's result was persisted, so the stored blocks
    /// are `[use A, use B, result A]`. `group_assistant_blocks` reads B as a
    /// permanent gap and synthesizes an is_error placeholder for it. Then the
    /// approval-resume path folds in B's freshly-executed REAL result.
    ///
    /// Pre-fix that fold blindly pushed a User message → TWO tool_result blocks with
    /// id B → `messages.N.content.M: each tool_use must have a single result`.
    #[test]
    fn resume_of_a_mixed_batch_yields_exactly_one_result_per_tool_use() {
        use crate::modules::chat::core::services::streaming::group_assistant_blocks;

        // What the DB holds at the pause (built-in A ran; B awaits approval).
        let mut request_messages = group_assistant_blocks(vec![
            tool_use("A", "builtin__remember"),
            tool_use("B", "rcpa__run_de_analysis"),
            tool_result("A", "remembered"),
        ]);

        // Precondition: the placeholder for B really is there (this is the setup the
        // bug needs — if this ever stops holding, the test is no longer the repro).
        assert_eq!(
            result_ids_all(&request_messages),
            vec!["A", "B"],
            "group_assistant_blocks synthesized a placeholder for the unapproved B"
        );
        assert!(
            result_text(&request_messages[1].content[1]).contains("Tool result unavailable"),
            "B's block starts life as the synthesized placeholder"
        );

        // CONTROL — the PRE-FIX behavior on this exact shape, to prove the test is
        // not vacuous. The old code appended every fresh result as a User message
        // unconditionally; on this shape that is the reported bug verbatim.
        // (That `assert_single_result_per_tool_use` actually CATCHES this is proven
        // separately by `invariant_assertion_catches_a_duplicate`, via #[should_panic]
        // — catching it inline would need a process-global panic-hook swap, which
        // would swallow the output of any genuinely failing test running in parallel.)
        {
            let mut pre_fix = request_messages.clone();
            pre_fix.push(ai_providers::ChatMessage {
                role: ai_providers::Role::User,
                content: vec![tool_result("B", "DE analysis complete: 412 genes")],
            });
            assert_eq!(
                result_ids_all(&pre_fix),
                vec!["A", "B", "B"],
                "CONTROL: blind-append (pre-fix) really does answer tool_use B twice — \
                 this is the shape the provider rejects"
            );
        }

        // The resume path folds in B's real result.
        let leftovers = replace_or_collect_tool_results(
            &mut request_messages,
            vec![tool_result("B", "DE analysis complete: 412 genes")],
        );
        if !leftovers.is_empty() {
            request_messages.push(ai_providers::ChatMessage {
                role: ai_providers::Role::User,
                content: leftovers,
            });
        }

        // (a) The invariant the provider enforces.
        assert_single_result_per_tool_use(&request_messages);
        assert_eq!(result_ids_all(&request_messages), vec!["A", "B"]);

        // (b) The SURVIVING B result is the real one, not the placeholder.
        assert_eq!(
            result_text(&request_messages[1].content[1]),
            "DE analysis complete: 412 genes",
            "the placeholder must be upgraded to the freshly-executed result"
        );

        // (c) Pairing still holds: results sit in the message immediately after the
        // Assistant turn that carries their tool_use (Anthropic requires both).
        assert!(matches!(
            request_messages[0].role,
            ai_providers::Role::Assistant
        ));
        assert!(matches!(request_messages[1].role, ai_providers::Role::Tool));
        assert_eq!(request_messages.len(), 2, "no trailing User message was needed");
    }

    /// TEST-13: the claim verdict. `delete_tool_approval` returns
    /// `Ok(rows_affected() > 0)`, and that bool IS the claim: `Ok(false)` means a
    /// concurrent pass already claimed the row and this one must NOT execute.
    /// Branching only on `Err` — discarding the bool — silently turns AlreadyClaimed
    /// into Won, i.e. a double-run of an approved, side-effecting tool.
    ///
    /// Honest limit: this pins the DECISION, not the wiring — it exercises the helper
    /// directly, so reverting the call site to a bool-discarding `if let Err(e)` would
    /// leave it green. Nothing at any tier can induce a losing/failing DELETE through
    /// the HTTP harness, so the call site's Won path is covered end-to-end by
    /// `mcp::approval_claim_test` and its non-Won paths are covered by construction:
    /// the `outcome != Won` branch is the single exit, and it always emits a result
    /// (the invariant TEST-18 shows is load-bearing).
    #[test]
    fn claim_outcome_distinguishes_won_already_claimed_and_failed() {
        assert_eq!(
            claim_outcome::<()>(Ok(true)),
            ClaimOutcome::Won,
            "we deleted the row — we own the execution"
        );
        assert_eq!(
            claim_outcome::<()>(Ok(false)),
            ClaimOutcome::AlreadyClaimed,
            "zero rows deleted means someone else claimed it — MUST NOT execute (this is \
             the case a bool-discarding claim silently gets wrong)"
        );
        assert_eq!(
            claim_outcome(Err(())),
            ClaimOutcome::Failed,
            "a failed DELETE leaves the row's fate unknown — fail loudly, never guess"
        );
    }

    /// Proves the invariant assertion used by the tests above is not vacuous: given a
    /// genuinely duplicated id it MUST panic. Pairs with the CONTROL block in
    /// `resume_of_a_mixed_batch_…`.
    #[test]
    #[should_panic(expected = "answered by MORE THAN ONE tool_result")]
    fn invariant_assertion_catches_a_duplicate() {
        assert_single_result_per_tool_use(&[ai_providers::ChatMessage {
            role: ai_providers::Role::Tool,
            content: vec![tool_result("B", "one"), tool_result("B", "two")],
        }]);
    }

    /// The current batch is the LAST one. An id reused by an OLDER turn (gpt-oss's
    /// constant `"tool_use"`) must not be found: overwriting it would corrupt that
    /// turn's history AND report no leftover, leaving the CURRENT tool_use unanswered.
    #[test]
    fn replace_or_collect_ignores_the_same_id_in_an_older_turn() {
        let mut msgs = vec![
            // Older, fully-resolved turn reusing the same id.
            ai_providers::ChatMessage {
                role: ai_providers::Role::Assistant,
                content: vec![tool_use("tool_use", "srv__search")],
            },
            ai_providers::ChatMessage {
                role: ai_providers::Role::Tool,
                content: vec![tool_result("tool_use", "turn-1 result")],
            },
            // Current batch: same id, awaiting approval → no result block yet.
            ai_providers::ChatMessage {
                role: ai_providers::Role::Assistant,
                content: vec![tool_use("tool_use", "srv__run_de_analysis")],
            },
        ];
        let leftovers =
            replace_or_collect_tool_results(&mut msgs, vec![tool_result("tool_use", "turn-2 real")]);

        assert_eq!(
            leftovers.len(),
            1,
            "the current tool_use has no result block yet, so the result must be appended"
        );
        assert_eq!(
            result_text(&msgs[1].content[0]),
            "turn-1 result",
            "the OLDER turn's result must not be overwritten"
        );
    }

    /// TEST-6: an existing block for the id is replaced IN PLACE — same message, same
    /// index, so adjacency to its tool_use is preserved — and nothing is left over.
    #[test]
    fn replace_or_collect_replaces_an_existing_result_in_place() {
        let mut msgs = vec![
            ai_providers::ChatMessage {
                role: ai_providers::Role::Assistant,
                content: vec![tool_use("B", "srv__b")],
            },
            ai_providers::ChatMessage {
                role: ai_providers::Role::Tool,
                content: vec![tool_result("B", "placeholder")],
            },
        ];
        let leftovers = replace_or_collect_tool_results(&mut msgs, vec![tool_result("B", "real")]);

        assert!(
            leftovers.is_empty(),
            "nothing to append — the result went into the existing slot"
        );
        assert_eq!(msgs.len(), 2, "no message added");
        assert_eq!(result_text(&msgs[1].content[0]), "real");
        assert_single_result_per_tool_use(&msgs);
    }

    /// TEST-7: with NO existing block for the id — the pure awaiting-approval batch,
    /// where `group_assistant_blocks` emits a bare Assistant turn and no Tool message
    /// — the result is returned for the User message and the request is untouched.
    /// This is the `chat-toolresult-pairing` regression guard: dropping this path
    /// would leave the tool_use unpaired.
    #[test]
    fn replace_or_collect_returns_a_result_with_no_existing_block() {
        let mut msgs = vec![ai_providers::ChatMessage {
            role: ai_providers::Role::Assistant,
            content: vec![tool_use("B", "srv__b")],
        }];
        let before = msgs.len();
        let leftovers = replace_or_collect_tool_results(&mut msgs, vec![tool_result("B", "real")]);

        assert_eq!(leftovers.len(), 1, "must be appended as a User message");
        assert_eq!(result_text(&leftovers[0]), "real");
        assert_eq!(msgs.len(), before, "request left untouched");
        assert!(result_ids_all(&msgs).is_empty());
    }

    /// TEST-8: a mixed fresh batch — one id has a placeholder, one does not — replaces
    /// the first in place and returns only the second.
    #[test]
    fn replace_or_collect_handles_a_mixed_batch() {
        let mut msgs = vec![
            ai_providers::ChatMessage {
                role: ai_providers::Role::Assistant,
                content: vec![tool_use("B", "srv__b"), tool_use("C", "srv__c")],
            },
            ai_providers::ChatMessage {
                role: ai_providers::Role::Tool,
                content: vec![tool_result("B", "placeholder")],
            },
        ];
        let leftovers = replace_or_collect_tool_results(
            &mut msgs,
            vec![tool_result("B", "b-real"), tool_result("C", "c-real")],
        );

        assert_eq!(leftovers.len(), 1, "only C had no existing block");
        assert_eq!(result_text(&leftovers[0]), "c-real");
        assert_eq!(result_text(&msgs[1].content[0]), "b-real", "B replaced in place");
        assert_single_result_per_tool_use(&msgs);
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
        // TEST-5: covers a file another tool HANDS you as a URL — use the ziee-provided /api/files
        // link, never forward the tool's raw upstream URL, never rewrite/substitute its host.
        assert!(with.contains("another tool HANDS you a file as a URL"), "{with}");
        assert!(with.contains("/api/files"), "{with}");
        assert!(with.contains("NEVER rewrite, guess, or substitute its host"), "{with}");

        // A different tool merely containing the substring must NOT trigger it
        // (suffix match guards against e.g. "get_resource_link_v2").
        let lookalike = tool_system_guidance(&[tool("abc__get_resource_link_v2")]);
        assert!(!lookalike.contains("you MUST first call get_resource_link"), "{lookalike}");
    }

    /// TEST-3 (stale-artifact-links): when get_resource_link is present, the system guidance
    /// must warn that its download URLs are short-lived and must be re-fetched each hand-off
    /// (never reuse an earlier-turn URL) — the fix for stale cross-turn artifact references.
    /// The note must be absent when get_resource_link isn't offered.
    #[test]
    fn guidance_marks_resource_link_urls_short_lived_and_refetch_each_turn() {
        let with = tool_system_guidance(&[tool(
            "11111111-2222-3333-4444-555555555555__get_resource_link",
        )]);
        assert!(with.contains("SHORT-LIVED"), "must flag URLs short-lived; {with}");
        assert!(
            with.contains("call get_resource_link again to obtain a FRESH URL"),
            "must instruct re-fetching a fresh URL each hand-off; {with}"
        );
        assert!(
            with.contains("never reuse a URL from an earlier turn"),
            "must forbid reusing an earlier-turn URL; {with}"
        );
        // No get_resource_link tool → no transience note (nothing to re-fetch).
        let without = tool_system_guidance(&[tool("abc__some_other_tool")]);
        assert!(!without.contains("SHORT-LIVED"), "{without}");
    }

    /// TEST-2 (stale-artifact-links): the shared saved-artifact hidden_content guidance must
    /// tell the model the download URLs are temporary and to re-obtain a fresh link
    /// (call get_resource_link) rather than reuse one from an earlier turn — and must NOT
    /// carry the old "do not call get_resource_link for these" instruction that caused the
    /// stale-URL reuse. It must embed the URL lines it is given.
    #[test]
    fn saved_artifact_guidance_is_transient_and_steers_refetch() {
        let url_lines = "genes.csv - http://127.0.0.1:8080/api/files/abc/download-with-token?token=t";
        let g = saved_artifact_hidden_content_guidance(url_lines);

        assert!(g.contains(url_lines), "must embed the passed URL lines; {g}");
        assert!(g.contains("TEMPORARY"), "must mark the URLs temporary; {g}");
        assert!(
            g.contains("re-obtain a fresh link") && g.contains("call get_resource_link"),
            "must steer the model to re-fetch a fresh link; {g}"
        );
        assert!(
            g.contains("never reuse a URL from an earlier turn"),
            "must forbid reusing an earlier-turn URL; {g}"
        );
        // The regression string that trained the model to reuse the stale URL must be gone.
        assert!(
            !g.to_lowercase().contains("do not call get_resource_link"),
            "must NOT tell the model to avoid get_resource_link; {g}"
        );
        // Preserve the anti-inline / anti-DRS / anti-localhost rules.
        assert!(
            g.contains("VERBATIM") && g.contains("DRS") && g.contains("127.0.0.1/localhost"),
            "must keep verbatim + anti-DRS/localhost rules; {g}"
        );
        // TEST-5: the saved list now explicitly covers a file another tool returned that ziee
        // re-hosted, steering the model to the ziee URL rather than the tool's upstream URL.
        assert!(
            g.contains("files another tool returned that ziee has re-hosted"),
            "must cover tool-returned re-hosted files; {g}"
        );
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
        let citations = crate::modules::citations::citations_server_id();
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
        // citations adds on top when ITS flag is set (the two mcp.rs edits — the
        // documented silent-failure footgun if forgotten).
        m.insert(crate::modules::citations::chat_extension::ATTACH_FLAG.into(), json!("true"));
        let with_cit = auto_attach_builtin_ids(&m);
        assert!(with_cit.contains(&citations), "citations flag must attach its server id");
        assert!(with_cit.contains(&lit) && with_cit.contains(&web));
        assert_eq!(with_cit.len(), 8);
        // A non-"true" flag value is ignored — only the always-on pair remains.
        let mut m2: HashMap<String, serde_json::Value> = HashMap::new();
        m2.insert("model_tools_capable".into(), json!(true));
        m2.insert("attach_files_mcp".into(), json!("false"));
        let only_base = auto_attach_builtin_ids(&m2);
        assert_eq!(only_base.len(), 2);
        assert!(always_on.iter().all(|id| only_base.contains(id)));
    }

    /// control_mcp attach seam (M7) + the security-critical negative (H8).
    /// control is auto-attached behind `attach_control_mcp` (the documented
    /// silent-failure footgun), but is deliberately NOT on the approval-bypass
    /// list — so a mutating `invoke_capability` is always forced through approval.
    #[test]
    fn control_attaches_on_flag_and_is_not_approval_bypassed() {
        let control = crate::modules::control_mcp::control_mcp_server_id();

        let mut m: HashMap<String, serde_json::Value> = HashMap::new();
        m.insert("model_tools_capable".into(), json!(true));
        assert!(
            !auto_attach_builtin_ids(&m).contains(&control),
            "control must not attach without its flag"
        );
        m.insert(
            crate::modules::control_mcp::chat_extension::ATTACH_FLAG.into(),
            json!("true"),
        );
        assert!(
            auto_attach_builtin_ids(&m).contains(&control),
            "attach_control_mcp must push the control server id (both mcp.rs edits)"
        );

        // The linchpin of "mutating writes require approval": if control were
        // ever added to is_builtin_server_id, its writes would auto-run.
        assert!(
            !is_builtin_server_id(control),
            "control must NOT be approval-bypassed"
        );
    }

    /// The three life-science built-ins (`bio_mcp`, `lit_search`, `citations`)
    /// must all attach TOGETHER when their flags are co-set on one tool-capable
    /// request, independently of the file/memory/web built-ins. `auto_attach_*`
    /// was only ever asserted cumulatively-on-top-of-everything before; this
    /// isolates the bio+lit+citations combination so a regression that made one
    /// flag clobber another (or that coupled bio/citations to web_search being
    /// on) would be caught. Mirrors mcp.rs:144-163.
    #[test]
    fn auto_attach_collects_bio_lit_citations_together() {
        let elicit = crate::modules::elicitation_mcp::elicitation_mcp_server_id();
        let tool_result = crate::modules::tool_result_mcp::tool_result_mcp_server_id();
        let bio = crate::modules::bio_mcp::bio_mcp_server_id();
        let lit = crate::modules::lit_search::lit_search_server_id();
        let citations = crate::modules::citations::citations_server_id();
        // Built-ins that are deliberately NOT flagged on this request.
        let files = crate::modules::files_mcp::files_mcp_server_id();
        let memory = crate::modules::memory_mcp::memory_mcp_server_id();
        let web = crate::modules::web_search::web_search_server_id();

        // Tool-capable model with ONLY the bio + lit_search + citations flags
        // set — no files/memory/web.
        let mut m: HashMap<String, serde_json::Value> = HashMap::new();
        m.insert("model_tools_capable".into(), json!(true));
        m.insert("attach_bio_mcp".into(), json!("true"));
        m.insert(
            crate::modules::lit_search::chat_extension::ATTACH_FLAG.into(),
            json!("true"),
        );
        m.insert(
            crate::modules::citations::chat_extension::ATTACH_FLAG.into(),
            json!("true"),
        );

        let ids = auto_attach_builtin_ids(&m);

        // All three life-science servers attach concurrently …
        assert!(ids.contains(&bio), "bio_mcp must attach");
        assert!(ids.contains(&lit), "lit_search must attach");
        assert!(ids.contains(&citations), "citations must attach");
        // … alongside the always-on pair …
        assert!(ids.contains(&elicit) && ids.contains(&tool_result));
        // … and the un-flagged built-ins stay OFF (no coupling / clobber).
        assert!(!ids.contains(&files));
        assert!(!ids.contains(&memory));
        assert!(!ids.contains(&web));
        // Exactly the 3 flagged + 2 always-on, no duplicates.
        assert_eq!(ids.len(), 5, "got {ids:?}");
        let mut deduped = ids.clone();
        deduped.sort();
        deduped.dedup();
        assert_eq!(deduped.len(), ids.len(), "no server id should appear twice");
    }

    #[test]
    fn elicitation_is_builtin_and_auto_approved() {
        // ask_user must be treated as a built-in so its tool skips the manual
        // approval prompt (the user answering the form IS the approval).
        assert!(is_builtin_server_id(
            crate::modules::elicitation_mcp::elicitation_mcp_server_id()
        ));
    }

    // TEST-21: the two run_js mcp.rs edits — the model SEES run_js (auto_attach)
    // and the script START auto-approves (is_builtin_server_id).
    #[test]
    fn run_js_auto_attach_and_builtin_seam() {
        let run_js_id = crate::modules::js_tool::run_js_mcp_server_id();
        // Approval-bypassed (script START runs without a prompt).
        assert!(is_builtin_server_id(run_js_id));
        // Attached when the flag is set (the model sees run_js).
        let mut md = std::collections::HashMap::new();
        md.insert("attach_run_js_mcp".to_string(), serde_json::json!("true"));
        assert!(auto_attach_builtin_ids(&md).contains(&run_js_id));
        // NOT attached without the flag.
        let empty = std::collections::HashMap::new();
        assert!(!auto_attach_builtin_ids(&empty).contains(&run_js_id));
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
        // citations (auto-attached; writes operate only on the caller's own
        // verified library) is approval-bypassed too.
        assert!(is_builtin_server_id(
            crate::modules::citations::citations_server_id()
        ));
        // A third-party server id is NOT a privileged built-in.
        assert!(!is_builtin_server_id(Uuid::new_v4()));
    }

    /// Cross-subsystem integration of the built-in MCP servers through the
    /// SHARED approval-bypass seam (`is_builtin_server_id`). This asserts the
    /// full matrix in one place — web_search + memory + lit_search + citations +
    /// elicitation + files + tool_result + bio + skill are all approval-bypassed
    /// together — and that the EXECUTION subsystems (code_sandbox, workflow) are
    /// deliberately NOT approval-bypassed (they run code / mutate, so a
    /// tool-capable chat that enables everything still gates them behind manual
    /// approval). Covers the "never tested together" cross-subsystem gaps.
    #[test]
    fn all_readonly_builtins_share_approval_bypass_but_execution_ones_do_not() {
        // Read-only / save-only / user-prompting built-ins: approval-bypassed.
        let bypassed = [
            crate::modules::memory_mcp::memory_mcp_server_id(),
            crate::modules::web_search::web_search_server_id(),
            crate::modules::lit_search::lit_search_server_id(),
            crate::modules::citations::citations_server_id(),
            crate::modules::elicitation_mcp::elicitation_mcp_server_id(),
            crate::modules::files_mcp::files_mcp_server_id(),
            crate::modules::tool_result_mcp::tool_result_mcp_server_id(),
            crate::modules::bio_mcp::bio_mcp_server_id(),
            crate::modules::skill_mcp::skill_mcp_server_id(),
        ];
        for id in bypassed {
            assert!(
                is_builtin_server_id(id),
                "read-only built-in {id} must be approval-bypassed"
            );
        }

        // Execution subsystems are NOT approval-bypassed — they mutate / run code
        // and must stay behind manual approval even when auto-attached.
        let needs_approval = [
            crate::modules::code_sandbox::code_sandbox_server_id(),
            crate::modules::workflow_mcp::workflow_mcp_server_id(),
        ];
        for id in needs_approval {
            assert!(
                !is_builtin_server_id(id),
                "execution built-in {id} must NOT be approval-bypassed"
            );
        }

        // And every id here is distinct (no accidental v5-namespace collision
        // between two subsystems that would conflate their privileges).
        let mut all: Vec<Uuid> = bypassed.to_vec();
        all.extend_from_slice(&needs_approval);
        let unique: std::collections::HashSet<_> = all.iter().collect();
        assert_eq!(unique.len(), all.len(), "built-in server ids must be unique");
    }
}

// Regression tests for the gpt-oss/harmony approval-loop fix: bare tool-name
// server_id recovery (ITEM-3) + message-unique tool_use ids (ITEM-2).
#[cfg(test)]
mod approval_loop_tests {
    use super::{
        recover_server_id_for_bare_name, resolve_server_and_tool, resolve_unique_tool_use_id,
    };
    use std::collections::{HashMap, HashSet};
    use uuid::Uuid;

    // resolve_server_and_tool — the wire-name → (server_id, tool_name) resolver.
    #[test]
    fn resolve_well_formed_uuid_prefix() {
        let sid = Uuid::new_v4();
        let map = HashMap::new();
        let (got_sid, tool) = resolve_server_and_tool(&format!("{sid}__echo"), &map);
        assert_eq!(got_sid, Some(sid));
        assert_eq!(tool, "echo");
    }

    #[test]
    fn resolve_well_formed_keeps_double_underscore_in_tool_name() {
        // `<uuid>__get__weather` splits on the FIRST `__` → tool name `get__weather`.
        let sid = Uuid::new_v4();
        let map = HashMap::new();
        let (got_sid, tool) = resolve_server_and_tool(&format!("{sid}__get__weather"), &map);
        assert_eq!(got_sid, Some(sid));
        assert_eq!(tool, "get__weather");
    }

    #[test]
    fn resolve_bare_name_recovers() {
        let sid = Uuid::new_v4();
        let mut map = HashMap::new();
        map.insert("execute_command".to_string(), Some(sid));
        let (got_sid, tool) = resolve_server_and_tool("execute_command", &map);
        assert_eq!(got_sid, Some(sid));
        assert_eq!(tool, "execute_command");
    }

    #[test]
    fn resolve_empty_prefix_recovers_remainder() {
        // gpt-oss/harmony `__query_rag`: advertised bare name is `query_rag`.
        let sid = Uuid::new_v4();
        let mut map = HashMap::new();
        map.insert("query_rag".to_string(), Some(sid));
        let (got_sid, tool) = resolve_server_and_tool("__query_rag", &map);
        assert_eq!(got_sid, Some(sid));
        assert_eq!(tool, "query_rag");
    }

    #[test]
    fn resolve_middle_double_underscore_is_not_stripped() {
        // `get__weather` (NOT advertised) must NOT be mis-dispatched to a different
        // server's `weather` tool — the middle `__` is part of the name.
        let other = Uuid::new_v4();
        let mut map = HashMap::new();
        map.insert("weather".to_string(), Some(other)); // a DIFFERENT tool/server
        let (got_sid, tool) = resolve_server_and_tool("get__weather", &map);
        assert_eq!(got_sid, None, "must not recover to the unrelated `weather` server");
        assert_eq!(tool, "get__weather");
    }

    #[test]
    fn resolve_middle_double_underscore_recovers_when_advertised() {
        // A genuine `get__weather` tool IS advertised → recovered as the whole name.
        let sid = Uuid::new_v4();
        let mut map = HashMap::new();
        map.insert("get__weather".to_string(), Some(sid));
        let (got_sid, tool) = resolve_server_and_tool("get__weather", &map);
        assert_eq!(got_sid, Some(sid));
        assert_eq!(tool, "get__weather");
    }

    #[test]
    fn resolve_unknown_bare_name_is_unresolved() {
        let map = HashMap::new();
        let (got_sid, tool) = resolve_server_and_tool("ghost_tool", &map);
        assert_eq!(got_sid, None);
        assert_eq!(tool, "ghost_tool");
    }

    // TEST-1 — an unambiguous bare name resolves to its single advertising server.
    #[test]
    fn recover_server_id_unambiguous_happy_path() {
        let sid = Uuid::new_v4();
        let mut map: HashMap<String, Option<Uuid>> = HashMap::new();
        map.insert("execute_command".to_string(), Some(sid));
        assert_eq!(
            recover_server_id_for_bare_name("execute_command", &map),
            Some(sid)
        );
    }

    // TEST-2 — a bare name advertised by ≥2 servers is marked ambiguous (`None`)
    // and is NOT auto-resolved (never guess a side-effecting tool's server).
    #[test]
    fn recover_server_id_ambiguous_returns_none() {
        let mut map: HashMap<String, Option<Uuid>> = HashMap::new();
        map.insert("execute_command".to_string(), None); // ambiguous sentinel
        assert_eq!(recover_server_id_for_bare_name("execute_command", &map), None);
    }

    // TEST-3 — an unknown bare name (absent from the advertised map) → None.
    #[test]
    fn recover_server_id_not_found_returns_none() {
        let map: HashMap<String, Option<Uuid>> = HashMap::new();
        assert_eq!(recover_server_id_for_bare_name("execute_command", &map), None);
    }

    // TEST-4 — an empty provider id mints a fresh, non-empty `call_<uuid>` id.
    #[test]
    fn resolve_id_mints_when_empty() {
        let used = HashSet::new();
        let id = resolve_unique_tool_use_id("", &used);
        assert!(!id.is_empty());
        assert!(id.starts_with("call_"), "{id}");
        // The suffix must be a valid UUID.
        assert!(Uuid::parse_str(id.trim_start_matches("call_")).is_ok(), "{id}");
    }

    // TEST-5 — a provider id already in `used` (within-batch OR cross-iteration
    // collision — both flow through `used`-membership) mints a fresh distinct id.
    #[test]
    fn resolve_id_mints_on_collision() {
        let mut used = HashSet::new();
        used.insert("tool_use".to_string());
        let id = resolve_unique_tool_use_id("tool_use", &used);
        assert_ne!(id, "tool_use");
        assert!(id.starts_with("call_"), "{id}");
        assert!(!used.contains(&id), "minted id must not already be taken");
    }

    // TEST-6 — a unique provider id not in `used` is preserved unchanged, so
    // well-behaved providers (Anthropic `toolu_…`, real OpenAI `call_…`) round-trip.
    #[test]
    fn resolve_id_preserves_good_provider_id() {
        let used = HashSet::new();
        assert_eq!(resolve_unique_tool_use_id("toolu_abc123", &used), "toolu_abc123");
        assert_eq!(
            resolve_unique_tool_use_id("chatcmpl-tool-90d1ec58ce2478f5", &used),
            "chatcmpl-tool-90d1ec58ce2478f5"
        );
    }
}

#[cfg(test)]
mod kb_builtin_tests {
    use super::is_builtin_server_id;
    use crate::modules::knowledge_base::knowledge_base_server_id;
    use uuid::Uuid;

    // TEST-18 (ITEM-21): the KB built-in server id is approval-bypassed (read-only
    // retrieval over the caller's own KBs); an arbitrary id is not.
    #[test]
    fn knowledge_base_id_is_a_builtin() {
        assert!(is_builtin_server_id(knowledge_base_server_id()));
        assert!(!is_builtin_server_id(Uuid::new_v4()));
    }
}

/// TEST-25 (ITEM-13): the two load-bearing predicates of the unattended
/// approval decision in the execute loop
/// (`needs_approval && unattended && !unattended_tool_allowed(..)` → Deny). These
/// tests exercise the REAL `unattended_tool_allowed` + `is_builtin_server_id`
/// (the exact inputs the decision consumes), not a reimplementation of the `if`.
#[cfg(test)]
mod scheduler_unattended_tests {
    use super::{is_builtin_server_id, unattended_tool_allowed};
    use serde_json::json;
    use uuid::Uuid;

    // The allow-list predicate: a whole-server grant (no `tool_name`) allows ANY
    // tool on that server; a per-tool grant allows only the named tool; a
    // different server / tool is never allowed; an empty list allows nothing.
    #[test]
    fn allow_list_matches_whole_server_and_specific_tool() {
        let srv = Uuid::new_v4().to_string();
        let other = Uuid::new_v4().to_string();

        // Empty allow-list → nothing is allowed.
        assert!(!unattended_tool_allowed(&json!([]), &srv, "anything"));

        // Whole-server grant → every tool on `srv`, but nothing on `other`.
        let whole = json!([{ "server_id": srv }]);
        assert!(unattended_tool_allowed(&whole, &srv, "foo"));
        assert!(unattended_tool_allowed(&whole, &srv, "bar"));
        assert!(!unattended_tool_allowed(&whole, &other, "foo"));

        // Per-tool grant → only the named tool on `srv`.
        let per_tool = json!([{ "server_id": srv, "tool_name": "foo" }]);
        assert!(unattended_tool_allowed(&per_tool, &srv, "foo"));
        assert!(!unattended_tool_allowed(&per_tool, &srv, "bar"));
        assert!(!unattended_tool_allowed(&per_tool, &other, "foo"));
    }

    // Decision semantics via the real predicate: in an unattended run, an
    // approval-required tool is DENIED iff it is NOT allow-listed. `Deny` ⇔
    // `!unattended_tool_allowed(..)`; adding a matching grant flips it back to
    // the ordinary (non-denied) approval path.
    #[test]
    fn unattended_denies_only_non_allow_listed_tools() {
        let srv = Uuid::new_v4().to_string();
        // Not allow-listed → the guard `!unattended_tool_allowed` is true → Deny.
        assert!(!unattended_tool_allowed(&json!([]), &srv, "delete_everything"));
        // Allow-listed (whole server) → guard false → NOT denied.
        let allow = json!([{ "server_id": srv }]);
        assert!(unattended_tool_allowed(&allow, &srv, "delete_everything"));
    }

    // A read-only built-in server is `is_builtin` → `needs_approval` is forced
    // false BEFORE the unattended check, so its tools ALWAYS bypass (never denied
    // even in an unattended run). A third-party id is not a built-in, so it flows
    // into the approval/deny decision above.
    #[test]
    fn readonly_builtins_bypass_the_unattended_gate() {
        assert!(is_builtin_server_id(
            crate::modules::files_mcp::files_mcp_server_id()
        ));
        assert!(is_builtin_server_id(
            crate::modules::memory_mcp::memory_mcp_server_id()
        ));
        assert!(is_builtin_server_id(
            crate::modules::tool_result_mcp::tool_result_mcp_server_id()
        ));
        // An arbitrary third-party server is NOT approval-bypassed → it is the
        // one subject to the unattended allow-list gate.
        assert!(!is_builtin_server_id(Uuid::new_v4()));
    }
}
