//! Skill chat extension — Path B (progressive disclosure).
//!
//! `before_llm_call` queries the available-skills view + injects ONE
//! system-message prefix listing each skill's name + description +
//! optional `when_to_use`. Bodies + supporting files are NEVER loaded
//! here — the LLM calls `skill_mcp`'s `load_skill` / `read_skill_file`
//! tools on demand. This keeps the always-loaded context cheap
//! (~50–100 tokens per skill; Agent Skills' 1536-char cap on
//! `description + when_to_use` bounds the worst case).
//!
//! Order **15** — between assistant (10) and memory (25). Confirmed
//! unoccupied per plan §4.5: 5 (text), 8 (project), 10 (assistant),
//! 20 (file), 24 (summarization), 25 (memory), 30 (mcp), 80 (title).


use ai_providers::{ChatMessage, ChatRequest, ContentBlock, Role};
use async_trait::async_trait;
use axum::response::sse::Event;
use linkme::distributed_slice;
use sqlx::PgPool;
use std::convert::Infallible;
use std::sync::Arc;

use crate::common::AppError;
use crate::core::Repos;
use crate::modules::chat::core::extension::{
    BeforeLlmAction, CHAT_EXTENSIONS, ChatExtension, ExtensionEntry, ExtensionMetadata,
    StreamContext,
};
use crate::modules::chat::core::extension::request::SendMessageRequest;

use super::super::repository::SkillAvailableEntry;

pub const METADATA: ExtensionMetadata = ExtensionMetadata {
    name: "skill",
    // 15 lands between assistant (10) and memory (25). The skill listing
    // is itself system-prompt scaffolding (akin to assistant + project
    // context) so it sits with the other early-stage injectors.
    order: 15,
};

const LISTING_HEADER: &str = "The following skills are available. Each is a procedural guide. \
When a user request matches a skill's description, call the `load_skill` tool (from the `skill_mcp` server) \
with the skill's name to read its full body. For supporting files, call `read_skill_file`.\n\n";

pub fn create(pool: PgPool, _config: Arc<crate::core::config::Config>) -> Arc<dyn ChatExtension> {
    Arc::new(SkillExtension::new(pool))
}

#[distributed_slice(CHAT_EXTENSIONS)]
static SKILL_EXTENSION: ExtensionEntry = ExtensionEntry {
    name: METADATA.name,
    order: METADATA.order,
    factory: create,
};

pub struct SkillExtension {
    // DB handle from the shared extension factory; the current hooks operate via
    // `context.pool`, so this owned handle is retained but unread today.
    #[allow(dead_code)]
    pool: PgPool,
}

impl SkillExtension {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl ChatExtension for SkillExtension {
    fn name(&self) -> &str {
        "skill"
    }

    async fn initialize(&self, _pool: &PgPool) -> Result<(), AppError> {
        tracing::info!("Skill chat extension initialized (Path B listing-only, order 15)");
        Ok(())
    }

    /// Query available skills + inject ONE system-message prefix. Bails
    /// silently on any error so a skill query failure can never break
    /// the chat path.
    async fn before_llm_call(
        &self,
        context: &mut StreamContext,
        request: &mut ChatRequest,
        _send_request: &SendMessageRequest,
        _tx: Option<&tokio::sync::mpsc::UnboundedSender<Result<Event, Infallible>>>,
    ) -> Result<BeforeLlmAction, AppError> {
        let entries = match Repos
            .skill
            .list_available_for_conversation(context.user_id, context.conversation_id)
            .await
        {
            Ok(v) => v,
            Err(e) => {
                tracing::warn!(
                    "skill.before_llm_call: list_available_for_conversation failed: {e}"
                );
                return Ok(BeforeLlmAction::Continue);
            }
        };

        if entries.is_empty() {
            return Ok(BeforeLlmAction::Continue);
        }

        let body = render_listing(&entries);
        request.messages.insert(
            0,
            ChatMessage {
                role: Role::System,
                content: vec![ContentBlock::Text { text: body }],
            },
        );

        // Attach the built-in `skill_mcp` server so the model can actually call
        // the `load_skill` / `read_skill_file` tools the listing tells it to
        // use — but only for tool-capable models (a non-tool model can't call
        // them). Mirrors the web_search / lit_search / citations flag contract;
        // `auto_attach_builtin_ids` reads `ATTACH_FLAG`.
        if crate::modules::file::available_files::model_supports_tools(&context.metadata).await {
            context
                .metadata
                .insert(super::ATTACH_FLAG.to_string(), serde_json::json!("true"));
        }

        Ok(BeforeLlmAction::Continue)
    }
}

/// Pure formatting helper — keeps the SQL out of the unit test, and
/// makes the wire format easy to lock down.
pub fn render_listing(entries: &[SkillAvailableEntry]) -> String {
    let mut out = String::from(LISTING_HEADER);
    for e in entries {
        out.push_str("- ");
        out.push_str(&e.name);
        if let Some(desc) = e.description.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
            out.push_str(" — ");
            out.push_str(desc);
        }
        if let Some(wtu) = e.when_to_use.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
            out.push_str(" [");
            out.push_str(wtu);
            out.push(']');
        }
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    fn entry(name: &str, description: Option<&str>, when_to_use: Option<&str>) -> SkillAvailableEntry {
        SkillAvailableEntry {
            id: Uuid::nil(),
            name: name.to_string(),
            description: description.map(String::from),
            when_to_use: when_to_use.map(String::from),
        }
    }

    #[test]
    fn header_present_and_skills_listed_with_separators() {
        let body = render_listing(&[
            entry("io.github.ziee/configure-llm-providers", Some("Add provider keys"), None),
            entry(
                "io.github.ziee/create-skill",
                Some("Author a new skill"),
                Some("when the user wants to add procedural guidance"),
            ),
        ]);
        assert!(body.starts_with("The following skills are available"));
        assert!(body.contains("`load_skill`"));
        assert!(body.contains("- io.github.ziee/configure-llm-providers — Add provider keys\n"));
        assert!(body.contains(
            "- io.github.ziee/create-skill — Author a new skill [when the user wants to add procedural guidance]\n"
        ));
    }

    #[test]
    fn empty_description_is_omitted_cleanly() {
        let body = render_listing(&[entry("io.github.ziee/bare", None, None)]);
        assert!(body.ends_with("- io.github.ziee/bare\n"));
        assert!(!body.contains(" — "));
        assert!(!body.contains("[]"));
    }

    #[test]
    fn whitespace_only_description_treated_as_empty() {
        let body = render_listing(&[entry("io.github.ziee/blank", Some("   "), Some(""))]);
        assert!(body.ends_with("- io.github.ziee/blank\n"));
        assert!(!body.contains(" — "));
        assert!(!body.contains("[]"));
    }
}
