//! Chat extension — tell the model where the mounted folders are.
//!
//! Layer 2 of "how does the model know the path of a mounted folder": a generic
//! `before_llm_call` hook that reads the runtime `SandboxMountProvider` registry
//! (fed by the desktop host-mount module; inert on standalone / remote-web) and
//! prepends a short system message listing each active mount's in-sandbox path
//! (`/mnt/<full host path>`) + mode. Pairs with the static `/mnt/...` convention
//! documented in the `execute_command` tool description (layer 1) — the
//! description teaches the deterministic mapping, this surfaces what is actually
//! mounted *now* so the model needn't probe `ls /mnt`.
//!
//! Feature-agnostic: it names "mounted folders", not "host folders" — whatever a
//! registered provider returns gets surfaced, and nothing is injected when
//! nothing is mounted (the common case, ~zero overhead: an empty registry read).


use std::sync::Arc;

use async_trait::async_trait;
use linkme::distributed_slice;
use sqlx::PgPool;

use ai_providers::{ChatMessage, ChatRequest, ContentBlock, Role};

use crate::common::AppError;
use crate::modules::chat::core::extension::{
    BeforeLlmAction, CHAT_EXTENSIONS, ChatExtension, ExtensionEntry, SendMessageRequest,
    StreamContext,
};
use crate::modules::code_sandbox::types::SandboxContext;
use crate::modules::code_sandbox::workflow_staging::{StageMode, StagedMount};
use crate::modules::code_sandbox::{backend, config, mount_provider};

/// Runs after project (8) / assistant (10); order is not load-bearing here (an
/// informational system note), it just keeps a stable, documented position.
const ORDER: i32 = 12;

const WRAPPER_OPEN: &str =
    "[Code sandbox — mounted folders. The user has mounted these host folders into \
     the sandbox; read them in place via execute_command at the exact paths below. Do \
     NOT upload, copy, or recreate them, and treat read-only mounts as immutable.]\n";
const WRAPPER_CLOSE: &str = "[End mounted folders]";

/// Pure builder: format the active mounts into a system message, or `None` when
/// nothing is mounted. Extracted so the wire format is unit-testable without a
/// DB or a registered provider.
pub fn mount_context_message(mounts: &[StagedMount]) -> Option<ChatMessage> {
    if mounts.is_empty() {
        return None;
    }
    let mut body = String::from(WRAPPER_OPEN);
    for m in mounts {
        let mode = match m.mode {
            StageMode::ReadOnly => "read-only",
            StageMode::ReadWrite => "read-write",
        };
        body.push_str("- ");
        body.push_str(&m.sandbox_path);
        body.push_str(" (");
        body.push_str(mode);
        body.push_str(")\n");
    }
    body.push_str(WRAPPER_CLOSE);
    Some(ChatMessage {
        role: Role::System,
        content: vec![ContentBlock::Text { text: body }],
    })
}

pub struct MountContextExtension;

#[async_trait]
impl ChatExtension for MountContextExtension {
    fn name(&self) -> &str {
        "code_sandbox_mounts"
    }

    async fn before_llm_call(
        &self,
        context: &mut StreamContext,
        request: &mut ChatRequest,
        _send_request: &SendMessageRequest,
        _tx: Option<
            &tokio::sync::mpsc::UnboundedSender<
                Result<axum::response::sse::Event, std::convert::Infallible>,
            >,
        >,
    ) -> Result<BeforeLlmAction, AppError> {
        // Sandbox disabled / not booted → nothing to mount, nothing to say.
        let Some(state) = config::get_state() else {
            return Ok(BeforeLlmAction::Continue);
        };
        // Mirror execute_command's apply gate: if the active backend can't bind
        // extra mounts, don't claim the folders are reachable.
        if !backend::active().supports_extra_mounts() {
            return Ok(BeforeLlmAction::Continue);
        }

        let ctx = SandboxContext {
            conversation_id: context.conversation_id,
            user_id: context.user_id,
            workspace: state
                .workspace_root
                .join(context.conversation_id.to_string()),
            files: Arc::new(Vec::new()),
        };

        // Same resolution + sanitization execute_command uses, so the paths the
        // model is told match the binds it will actually get (skip-missing too).
        let (mounts, _notes) = mount_provider::collect_and_sanitize(&ctx).await;
        if let Some(msg) = mount_context_message(&mounts) {
            tracing::debug!(
                conversation_id = %context.conversation_id,
                mount_count = mounts.len(),
                "code_sandbox: injecting mounted-folder context into LLM call"
            );
            // Front of the message list — environment framing, like the other
            // context-injecting extensions' system blocks.
            request.messages.insert(0, msg);
        }
        Ok(BeforeLlmAction::Continue)
    }
}

fn create(_pool: PgPool, _config: Arc<crate::core::config::Config>) -> Arc<dyn ChatExtension> {
    Arc::new(MountContextExtension)
}

#[distributed_slice(CHAT_EXTENSIONS)]
static CODE_SANDBOX_MOUNTS_EXTENSION: ExtensionEntry = ExtensionEntry {
    name: "code_sandbox_mounts",
    order: ORDER,
    factory: create,
};

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn mount(path: &str, mode: StageMode) -> StagedMount {
        StagedMount {
            mode,
            host_path: PathBuf::from("/host/whatever"),
            sandbox_path: path.to_string(),
        }
    }

    fn text_of(msg: &ChatMessage) -> &str {
        match &msg.content[0] {
            ContentBlock::Text { text } => text,
            _ => panic!("expected a text content block"),
        }
    }

    #[test]
    fn no_mounts_injects_nothing() {
        assert!(mount_context_message(&[]).is_none());
    }

    #[test]
    fn lists_each_mount_path_and_mode_as_a_system_block() {
        let msg = mount_context_message(&[
            mount("/mnt/Users/me/runs/run5", StageMode::ReadOnly),
            mount("/mnt/data/refs", StageMode::ReadWrite),
        ])
        .expect("non-empty mounts produce a system message");

        assert_eq!(msg.role, Role::System);
        let text = text_of(&msg);
        // The exact, reversible /mnt/<full host path> each mount lands at.
        assert!(text.contains("/mnt/Users/me/runs/run5 (read-only)"));
        assert!(text.contains("/mnt/data/refs (read-write)"));
        // Wrapped so the model can tell our framing from the listing.
        assert!(text.starts_with("[Code sandbox"));
        assert!(text.ends_with("[End mounted folders]"));
    }

    #[test]
    fn read_only_and_read_write_render_distinct_modes() {
        let ro = mount_context_message(&[mount("/mnt/a", StageMode::ReadOnly)]).unwrap();
        assert!(text_of(&ro).contains("(read-only)"));
        let rw = mount_context_message(&[mount("/mnt/a", StageMode::ReadWrite)]).unwrap();
        assert!(text_of(&rw).contains("(read-write)"));
    }
}
