//! `SummarizationExtension` — wires the apply-summary (before_llm_call)
//! and refresh-summary (after_llm_call) hooks into the chat stream.
//!
//! Order 24 (declared in `extension.rs`) runs this BEFORE the memory
//! extension at order 25 — the summary block lands first, then
//! memory's retrieval block is appended to the compacted history.

use std::collections::HashMap;

use aide::axum::ApiRouter;
use async_trait::async_trait;
use axum::response::sse::Event;
use sqlx::PgPool;
use std::convert::Infallible;

use ai_providers::ChatRequest;

use crate::common::AppError;
use crate::core::Repos;
use crate::modules::chat::core::extension::{
    BeforeLlmAction, ChatExtension, ExtensionAction, StreamContext,
};
use crate::modules::chat::core::extension::request::SendMessageRequest;
use crate::modules::chat::core::models::Message;

pub struct SummarizationExtension {
    #[allow(dead_code)]
    pool: PgPool,
}

impl SummarizationExtension {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

/// Pull the conversation's chat-model id out of `StreamContext.metadata`.
/// Lives at module level (not inside the impl) so it's unit-testable
/// without building a `StreamContext` — the zero-config fallback is the
/// single decision the rest of the extension can't observe through
/// integration tests cheaply. Returns None when:
///   - the metadata map doesn't carry `"model_id"` (chat wired it
///     differently for some flow),
///   - the value isn't a string,
///   - the string isn't a UUID.
fn conversation_model_id(metadata: &HashMap<String, serde_json::Value>) -> Option<uuid::Uuid> {
    metadata
        .get("model_id")
        .and_then(|v| v.as_str())
        .and_then(|s| uuid::Uuid::parse_str(s).ok())
}

/// Resolve the effective enabled state for one conversation:
///   per-conversation `on`  → true
///   per-conversation `off` → false
///   per-conversation `inherit` or absent → `admin_enabled`
fn resolve_effective_enabled(per_conv_mode: &str, admin_enabled: bool) -> bool {
    match per_conv_mode {
        "on" => true,
        "off" => false,
        _ => admin_enabled,
    }
}

#[async_trait]
impl ChatExtension for SummarizationExtension {
    fn name(&self) -> &str {
        "summarization"
    }

    async fn initialize(&self, _pool: &PgPool) -> Result<(), AppError> {
        tracing::info!("Summarization extension initialized");
        Ok(())
    }

    /// Apply the persisted summary block to the prompt assembly. Drops
    /// the summarized prefix of `chat_request.messages` and replaces
    /// it with the summary system block.
    ///
    /// Fail-soft on every failure path — summarization must NEVER break
    /// chat. A transient DB blip on the per-conv-mode read defaults to
    /// `inherit`; a blip on the admin-settings read defaults to
    /// `enabled = true`. If BOTH read fail, the effective decision is
    /// `enabled = true` (on-by-default). In that combined-outage path
    /// the downstream `apply_summary_to_history` also fail-softs
    /// (returns Ok(None) on its own fetch failure), so the user just
    /// gets a chat turn without a summary block — the safer choice
    /// than aborting the turn entirely.
    async fn before_llm_call(
        &self,
        context: &mut StreamContext,
        request: &mut ChatRequest,
        _send_request: &SendMessageRequest,
        _tx: Option<&tokio::sync::mpsc::UnboundedSender<Result<Event, Infallible>>>,
    ) -> Result<BeforeLlmAction, AppError> {
        // Resolve effective-on for this conversation. Failures fall
        // back to "enabled" — the admin row defaults to TRUE in
        // migration 91, and the missing per-conversation row defaults
        // to `inherit`, so this is the on-by-default path.
        let per_conv_mode = Repos
            .chat
            .summarization
            .get_conversation_summarization_mode(context.conversation_id)
            .await
            .unwrap_or_else(|_| {
                super::repository::DEFAULT_SUMMARIZATION_MODE.to_string()
            });
        let admin_enabled = Repos
            .summarization
            .get_admin_settings()
            .await
            .map(|a| a.enabled)
            .unwrap_or(true);
        if !resolve_effective_enabled(&per_conv_mode, admin_enabled) {
            return Ok(BeforeLlmAction::Continue);
        }

        if let Err(e) = crate::modules::summarization::engine::summarizer::apply_summary_to_history(
            context.branch_id,
            request,
        )
        .await
        {
            tracing::warn!("summarization.before_llm_call: apply failed: {e}");
        }

        Ok(BeforeLlmAction::Continue)
    }

    /// Refresh the rolling summary if the branch crossed the trigger.
    /// All DB reads happen INSIDE the spawn (audit lesson: keep the
    /// hot path of the chat turn clean), and errors are logged not
    /// `?`-propagated.
    async fn after_llm_call(
        &self,
        context: &StreamContext,
        _final_message: &Message,
        _tx: Option<&tokio::sync::mpsc::UnboundedSender<Result<Event, Infallible>>>,
    ) -> Result<ExtensionAction, AppError> {
        let branch_id = context.branch_id;
        let conversation_id = context.conversation_id;
        let metadata = context.metadata.clone();

        tokio::spawn(async move {
            // Resolve effective-enabled inside the spawn.
            let per_conv_mode = Repos
                .chat
                .summarization
                .get_conversation_summarization_mode(conversation_id)
                .await
                .unwrap_or_else(|_| {
                    super::repository::DEFAULT_SUMMARIZATION_MODE.to_string()
                });
            let admin = match Repos.summarization.get_admin_settings().await {
                Ok(a) => a,
                Err(_) => return,
            };
            if !resolve_effective_enabled(&per_conv_mode, admin.enabled) {
                return;
            }

            // Cheap guard: skip brand-new branches.
            let history_count = match Repos.chat.core.get_conversation_history(branch_id).await {
                Ok(h) => h.len(),
                Err(_) => return,
            };
            if history_count < 4 {
                return;
            }

            // Zero-config fallback: NULL admin model → conversation's model.
            let model_id = admin
                .default_summarization_model_id
                .or_else(|| conversation_model_id(&metadata));
            let Some(model_id) = model_id else { return; };

            // Fraction-of-window: clamp the flat summary trigger by 0.75× the
            // chat model's context window so a small-context local model
            // summarizes before it overflows. None when the window is unknown
            // → flat admin threshold. Resolved INSIDE the spawn so the chat
            // turn's hot path stays clean (audit lesson).
            let trigger_override: Option<usize> =
                crate::modules::file::available_files::model_context_window(&metadata)
                    .await
                    .map(|w| (w as f64 * 0.75) as usize);

            if let Err(e) = crate::modules::summarization::engine::summarizer::refresh_summary(
                branch_id,
                model_id,
                trigger_override,
            )
            .await
            {
                tracing::warn!(
                    "summarization.after_llm_call: refresh failed for branch {branch_id}: {e}"
                );
            }
        });

        Ok(ExtensionAction::Complete)
    }

    fn register_routes(&self, router: ApiRouter) -> ApiRouter {
        router.merge(super::summarization_mode_routes::summarization_mode_router())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conversation_model_id_present_and_valid() {
        let mut m = HashMap::new();
        let id = uuid::Uuid::new_v4();
        m.insert("model_id".to_string(), serde_json::json!(id.to_string()));
        assert_eq!(conversation_model_id(&m), Some(id));
    }

    #[test]
    fn conversation_model_id_missing_returns_none() {
        let m = HashMap::new();
        assert_eq!(conversation_model_id(&m), None);
    }

    #[test]
    fn conversation_model_id_non_string_returns_none() {
        let mut m = HashMap::new();
        m.insert("model_id".to_string(), serde_json::json!(42));
        assert_eq!(conversation_model_id(&m), None);
    }

    #[test]
    fn conversation_model_id_malformed_uuid_returns_none() {
        let mut m = HashMap::new();
        m.insert("model_id".to_string(), serde_json::json!("not-a-uuid"));
        assert_eq!(conversation_model_id(&m), None);
    }

    #[test]
    fn resolve_effective_enabled_per_conv_on_overrides_admin_off() {
        assert!(resolve_effective_enabled("on", false));
    }

    #[test]
    fn resolve_effective_enabled_per_conv_off_overrides_admin_on() {
        assert!(!resolve_effective_enabled("off", true));
    }

    #[test]
    fn resolve_effective_enabled_inherit_follows_admin() {
        assert!(resolve_effective_enabled("inherit", true));
        assert!(!resolve_effective_enabled("inherit", false));
    }

    /// Fail-soft contract: on a DB error before_llm_call defaults per_conv_mode
    /// to DEFAULT_SUMMARIZATION_MODE and admin_enabled to `true`
    /// (`.unwrap_or_else(|_| DEFAULT)` + `.unwrap_or(true)`). Those exact
    /// defaults MUST compose to the on-by-default path, so a transient DB
    /// failure degrades to "summarization enabled", not silently off.
    #[test]
    fn fail_soft_defaults_resolve_to_enabled() {
        // The documented default mode is the inherit path.
        assert_eq!(super::super::repository::DEFAULT_SUMMARIZATION_MODE, "inherit");
        // DB-error defaults: mode = DEFAULT_SUMMARIZATION_MODE, admin = true.
        assert!(resolve_effective_enabled(
            super::super::repository::DEFAULT_SUMMARIZATION_MODE,
            true
        ));
    }
}
