//! code_sandbox chat-extension bridge.
//!
//! `extension.rs` registers via `linkme::distributed_slice(CHAT_EXTENSIONS)`.
//! This module's `CodeSandboxAttachExtension` is the `ChatExtension` impl: its
//! `before_llm_call` sets the `attach_code_sandbox` metadata flag (read by
//! `auto_attach_builtin_ids` in `mcp/chat_extension/mcp.rs`) when code_sandbox is
//! enabled AND the model is tool-capable.
//!
//! Why this exists: `code_sandbox` registers its `execute_command` MCP tool and
//! works over direct MCP, but without this flag the tool is NEVER advertised to a
//! chat — so a tool-capable model (told about `execute_command` by the always-on
//! `spawn_background` description) calls it and gets "could not resolve an MCP
//! server". This is the silent-failure class CLAUDE.md §11 warns about (the FIRST
//! of the two `mcp.rs` edits was missing). The SECOND edit (`is_builtin_server_id`)
//! is deliberately NOT applied — `execute_command` runs code and must stay behind
//! manual approval (like `control` / `background` / `workflow`).

pub mod extension;

use std::collections::HashMap;
use std::convert::Infallible;

use async_trait::async_trait;
use axum::response::sse::Event;

use ai_providers::ChatRequest;

use crate::common::AppError;
use crate::modules::chat::core::extension::request::SendMessageRequest;
use crate::modules::chat::core::extension::{BeforeLlmAction, ChatExtension, StreamContext};

/// Metadata flag set by this extension's `before_llm_call` and read by
/// `mcp::chat_extension::auto_attach_builtin_ids`. Shared as one const so a typo
/// can't silently desync the producer from the consumer (the documented
/// silent-failure point in CLAUDE.md §11).
pub const ATTACH_FLAG: &str = "attach_code_sandbox";

/// Pure gating decision: attach code_sandbox to a chat iff the model is
/// tool-capable AND the sandbox is enabled+initialized. Extracted as a pure
/// function so the enabled/disabled × tool-capable matrix (the INV-1 promise) is
/// directly unit-testable without a live sandbox or a running server.
pub fn should_attach(tool_capable: bool, sandbox_enabled: bool) -> bool {
    tool_capable && sandbox_enabled
}

/// Set the auto-attach flag. Pure (operates on the passed-in metadata map) so the
/// producer/consumer contract point — the documented silent-failure spot — is
/// directly unit-testable. The flag key is the shared [`ATTACH_FLAG`] const that
/// `auto_attach_builtin_ids` reads, so the producer/consumer can't desync.
fn apply_code_sandbox_attach(metadata: &mut HashMap<String, serde_json::Value>) {
    metadata.insert(ATTACH_FLAG.to_string(), serde_json::json!("true"));
}

/// The producer wiring, pure over its inputs: set the attach flag iff eligible.
/// Extracted so `before_llm_call`'s glue — "when eligible, actually CALL apply" —
/// is machine-verified without a live sandbox (a producer that gated on the wrong
/// condition, or forgot to call apply, fails the enabled-path test). This is the
/// positive INV-1 half; the disabled half is proven end-to-end by the integration
/// test. `before_llm_call` supplies the real inputs (`model_supports_tools` and
/// `config::get_state().is_some()`).
pub fn apply_attach_if_eligible(
    metadata: &mut HashMap<String, serde_json::Value>,
    tool_capable: bool,
    sandbox_enabled: bool,
) {
    if should_attach(tool_capable, sandbox_enabled) {
        apply_code_sandbox_attach(metadata);
    }
}

pub struct CodeSandboxAttachExtension;

#[async_trait]
impl ChatExtension for CodeSandboxAttachExtension {
    fn name(&self) -> &str {
        "code_sandbox_attach"
    }

    async fn before_llm_call(
        &self,
        context: &mut StreamContext,
        _request: &mut ChatRequest,
        _send_request: &SendMessageRequest,
        _tx: Option<&tokio::sync::mpsc::UnboundedSender<Result<Event, Infallible>>>,
    ) -> Result<BeforeLlmAction, AppError> {
        // Cheapest gate first: a non-tool-capable model can't call the tool, so
        // don't attach.
        let tool_capable =
            crate::modules::file::available_files::model_supports_tools(&context.metadata).await;
        // Enabled predicate: `get_state()` is `Some` only when code_sandbox is
        // enabled AND workspace init succeeded (mirrors `mount_context_extension` +
        // `client/stdio.rs`). A disabled deployment never attaches.
        let sandbox_enabled = crate::modules::code_sandbox::config::get_state().is_some();

        apply_attach_if_eligible(&mut context.metadata, tool_capable, sandbox_enabled);

        Ok(BeforeLlmAction::Continue)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // TEST-1 — apply_code_sandbox_attach sets the SHARED flag key to "true".
    #[test]
    fn apply_attach_sets_shared_flag() {
        let mut md: HashMap<String, serde_json::Value> = HashMap::new();
        apply_code_sandbox_attach(&mut md);
        assert_eq!(md.get(ATTACH_FLAG).and_then(|v| v.as_str()), Some("true"));
    }

    // TEST-2 [acceptance] [invariant: INV-1] — the gating decision is TRUE only
    // when the model is tool-capable AND the sandbox is enabled; FALSE otherwise.
    // This is INV-1's promise: enabled+tool-capable ⇒ flag set (⇒ advertised);
    // disabled ⇒ flag not set (⇒ not advertised). `before_llm_call` binds
    // `sandbox_enabled` to `config::get_state().is_some()` and `tool_capable` to
    // `model_supports_tools`.
    #[test]
    fn should_attach_only_when_tool_capable_and_enabled() {
        assert!(should_attach(true, true), "tool-capable + enabled ⇒ attach");
        assert!(
            !should_attach(true, false),
            "disabled ⇒ NOT advertised even for a tool-capable model"
        );
        assert!(
            !should_attach(false, true),
            "non-tool-capable ⇒ NOT advertised even when enabled"
        );
        assert!(!should_attach(false, false), "neither ⇒ not attached");
    }

    // TEST-7 [acceptance] [invariant: INV-1] — the producer WIRING: the glue
    // `before_llm_call` runs actually SETS the flag when eligible and leaves it
    // unset otherwise. A producer that gated on the wrong condition or forgot to
    // call `apply` fails this — closing the "enabled path never exercised" gap the
    // piecewise should_attach/apply tests leave open (rootfs-free).
    #[test]
    fn apply_attach_if_eligible_sets_flag_only_when_eligible() {
        // Eligible (tool-capable + enabled) ⇒ flag SET (the positive INV-1 half).
        let mut on: HashMap<String, serde_json::Value> = HashMap::new();
        apply_attach_if_eligible(&mut on, true, true);
        assert_eq!(
            on.get(ATTACH_FLAG).and_then(|v| v.as_str()),
            Some("true"),
            "enabled + tool-capable ⇒ producer must set the attach flag"
        );

        // Disabled ⇒ flag ABSENT.
        let mut off: HashMap<String, serde_json::Value> = HashMap::new();
        apply_attach_if_eligible(&mut off, true, false);
        assert!(
            !off.contains_key(ATTACH_FLAG),
            "disabled ⇒ producer must NOT set the flag"
        );

        // Not tool-capable ⇒ flag ABSENT.
        let mut nc: HashMap<String, serde_json::Value> = HashMap::new();
        apply_attach_if_eligible(&mut nc, false, true);
        assert!(
            !nc.contains_key(ATTACH_FLAG),
            "non-tool-capable ⇒ producer must NOT set the flag"
        );
    }
}
