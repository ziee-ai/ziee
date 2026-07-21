//! The unified structured summarizer (ITEM-56 / ITEM-60 — DEC-127/128).
//!
//! Before this module there were TWO summarization prompts + call paths: the
//! server's rolling `conversation_summaries` engine (freeform "3-6 sentence
//! narrative") and the agent-core [`Compactor`](crate::compaction::Compactor)'s
//! Tier-4 (an inline freeform "summarize concisely" prompt). DEC-127 replaces
//! **both** freeform prompts with ONE structured **nine-section** template, and
//! DEC-128 makes the summarize step a single shared seam the [`Compactor`]
//! delegates to instead of hand-rolling its own prompt + model call.
//!
//! This module is the canonical home of that seam:
//! - [`SUMMARY_PROMPT_9_SECTION`] — the ONE prompt, as DATA (a `const`), shared
//!   across crates (the server engine sources its default template from it).
//! - [`Summarizer`] — the port: "given a transcript (+ an optional previous
//!   rolling summary), produce a structured nine-section summary".
//! - [`ModelSummarizer`] — the default impl over an injected
//!   [`ModelClient`](crate::core::ModelClient); what the [`Compactor`] uses on
//!   both the chat-agent and workflow paths (and in tests). A host MAY inject a
//!   different [`Summarizer`] (e.g. an engine-backed one that also persists to
//!   `conversation_summaries`) without touching the pipeline.
//!
//! The prompt is domain-free: it names no server-specific tool (Section 7 says
//! "recall handles" generically; the host maps that onto `get_tool_result`).

use std::sync::Arc;

use ai_providers::{ChatMessage, ChatRequest, ContentBlock};
use async_trait::async_trait;
use ziee_core::AppError;

use crate::core::ModelClient;

/// The single structured summary prompt (DEC-127): NINE sections that "replace
/// both freeform" prompts. Kept as DATA so there is exactly one place the
/// summary format lives — the server engine sources its default template from
/// this const, and [`ModelSummarizer`] sends it as the system message.
///
/// Sections (in order): user requests/intent · task list verbatim · decisions ·
/// files/edit state · errors/fixes · WIP/next · recall handles · governance
/// signals · durable facts.
pub const SUMMARY_PROMPT_9_SECTION: &str = r#"You are compacting a long agent/assistant conversation to free up context while losing nothing load-bearing. Write a DENSE, structured summary organized under the following NINE sections. Emit every header in order; if a section has nothing, write "(none)". Preserve specifics verbatim — identifiers, names, file paths, numbers, and tool_use ids. Do not editorialize. Output only the summary.

1. USER REQUESTS & INTENT — every distinct thing the user asked for and the overall goal, in the order raised.
2. TASK LIST — the current task / to-do list VERBATIM, including each item's status; do not reword or reorder items.
3. DECISIONS — choices made and their rationale (approaches adopted, alternatives rejected and why).
4. FILES & EDIT STATE — files created / modified / read and their current state (what changed, what remains to do).
5. ERRORS & FIXES — errors, failures, or test breakages encountered and how each was resolved (or that it is still open).
6. WORK IN PROGRESS / NEXT STEPS — what was mid-flight when compaction ran and the concrete next actions.
7. RECALL HANDLES — references to evicted tool results or artifacts still needed (e.g. get_tool_result ids, run-scoped resource refs) so they can be re-fetched on demand.
8. GOVERNANCE SIGNALS — approvals granted or denied, risk classifications, and any policy / permission constraints in effect.
9. DURABLE FACTS — stable facts about the user, environment, or domain worth carrying forward across the whole run."#;

/// Input to a [`Summarizer`] call. `transcript` is the role-tagged text of the
/// earlier messages being compacted; `previous_summary`, when set, turns the
/// call into an INCREMENTAL refresh (fold the new turns into an existing
/// rolling summary) rather than a full re-summarize.
#[derive(Debug, Clone, Default)]
pub struct SummaryInput {
    /// Role-tagged transcript of the messages being summarized (full path) or of
    /// only the new turns since the last refresh (incremental path).
    pub transcript: String,
    /// An existing rolling summary to update in place. `None`/empty → full.
    pub previous_summary: Option<String>,
}

/// The ONE summarization seam (DEC-128). The [`Compactor`](crate::compaction::Compactor)'s
/// Tier-4 delegates here instead of building its own prompt + model call, so the
/// nine-section format and the model invocation live in a single place.
#[async_trait]
pub trait Summarizer: Send + Sync {
    /// Produce a structured nine-section summary of `input`.
    async fn summarize(&self, input: SummaryInput) -> Result<String, AppError>;
}

/// Assemble the summary [`ChatRequest`] from the shared nine-section template.
/// The instruction body ([`SUMMARY_PROMPT_9_SECTION`]) is the system message; the
/// transcript (and, for an incremental refresh, the previous summary) rides in
/// the user message. Pure — unit-testable without a model.
pub fn build_summary_request(model_name: &str, input: &SummaryInput) -> ChatRequest {
    let user = match input.previous_summary.as_deref() {
        Some(prev) if !prev.trim().is_empty() => format!(
            "An existing running summary is below, followed by the new conversation turns since it. \
             Produce an UPDATED summary in the SAME nine-section format: fold in the new turns and \
             drop anything no longer relevant. Keep every section header.\n\n\
             Existing summary:\n{prev}\n\nNew conversation turns since the existing summary:\n{}",
            input.transcript
        ),
        _ => format!("Conversation to summarize:\n{}", input.transcript),
    };
    ChatRequest {
        model: model_name.to_string(),
        messages: vec![
            ChatMessage::system(SUMMARY_PROMPT_9_SECTION),
            ChatMessage::user(user),
        ],
        // Low temperature (deterministic recap) + room for a dense nine-section
        // block; a summary carries no tool_use, so no tools/tool_choice.
        temperature: Some(0.3),
        max_tokens: Some(1200),
        ..Default::default()
    }
}

/// The default [`Summarizer`]: render the shared nine-section prompt and call an
/// injected [`ModelClient`]. What the [`Compactor`](crate::compaction::Compactor)
/// wraps for both the chat-agent and workflow paths (and every unit test).
pub struct ModelSummarizer {
    model: Arc<dyn ModelClient>,
    /// The model name written into the summary [`ChatRequest`].
    model_name: String,
}

impl ModelSummarizer {
    pub fn new(model: Arc<dyn ModelClient>, model_name: impl Into<String>) -> Self {
        Self {
            model,
            model_name: model_name.into(),
        }
    }
}

#[async_trait]
impl Summarizer for ModelSummarizer {
    async fn summarize(&self, input: SummaryInput) -> Result<String, AppError> {
        let req = build_summary_request(&self.model_name, &input);
        let (msg, _usage) = self.model.call(req).await?;
        Ok(msg
            .content
            .iter()
            .filter_map(|b| match b {
                ContentBlock::Text { text } => Some(text.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join(""))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_fakes::ScriptedModel;

    /// The nine-section headers the prompt MUST carry — the ITEM-60 contract.
    const SECTION_HEADERS: [&str; 9] = [
        "USER REQUESTS & INTENT",
        "TASK LIST",
        "DECISIONS",
        "FILES & EDIT STATE",
        "ERRORS & FIXES",
        "WORK IN PROGRESS / NEXT STEPS",
        "RECALL HANDLES",
        "GOVERNANCE SIGNALS",
        "DURABLE FACTS",
    ];

    #[test]
    fn prompt_carries_all_nine_sections_in_order() {
        let mut last = 0usize;
        for (n, header) in SECTION_HEADERS.iter().enumerate() {
            let at = SUMMARY_PROMPT_9_SECTION
                .find(header)
                .unwrap_or_else(|| panic!("section {} header missing: {header}", n + 1));
            assert!(at >= last, "section {} out of order: {header}", n + 1);
            last = at;
            // Each section is numbered "N." in order.
            assert!(SUMMARY_PROMPT_9_SECTION.contains(&format!("{}. {header}", n + 1)));
        }
    }

    #[test]
    fn full_request_uses_nine_section_system_prompt_and_carries_transcript() {
        let input = SummaryInput {
            transcript: "user: hi\nassistant: there".into(),
            previous_summary: None,
        };
        let req = build_summary_request("m", &input);
        assert_eq!(req.model, "m");
        // System message IS the shared nine-section const.
        match &req.messages[0].content[0] {
            ContentBlock::Text { text } => assert_eq!(text, SUMMARY_PROMPT_9_SECTION),
            other => panic!("expected text system prompt, got {other:?}"),
        }
        // User message carries the transcript (full path — no "Existing summary").
        let user = match &req.messages[1].content[0] {
            ContentBlock::Text { text } => text.as_str(),
            other => panic!("expected text user msg, got {other:?}"),
        };
        assert!(user.contains("user: hi"));
        assert!(!user.contains("Existing summary"));
        assert_eq!(req.temperature, Some(0.3));
        assert_eq!(req.max_tokens, Some(1200));
    }

    #[test]
    fn incremental_request_carries_previous_summary_and_new_turns() {
        let input = SummaryInput {
            transcript: "user: new stuff".into(),
            previous_summary: Some("prior recap".into()),
        };
        let req = build_summary_request("m", &input);
        // Still the same nine-section system prompt (one format, both paths).
        match &req.messages[0].content[0] {
            ContentBlock::Text { text } => assert_eq!(text, SUMMARY_PROMPT_9_SECTION),
            other => panic!("expected text system prompt, got {other:?}"),
        }
        let user = match &req.messages[1].content[0] {
            ContentBlock::Text { text } => text.as_str(),
            other => panic!("expected text user msg, got {other:?}"),
        };
        assert!(user.contains("Existing summary:\nprior recap"));
        assert!(user.contains("user: new stuff"));
    }

    #[test]
    fn empty_previous_summary_is_treated_as_full() {
        let input = SummaryInput {
            transcript: "user: hi".into(),
            previous_summary: Some("   ".into()),
        };
        let req = build_summary_request("m", &input);
        let user = match &req.messages[1].content[0] {
            ContentBlock::Text { text } => text.as_str(),
            other => panic!("expected text, got {other:?}"),
        };
        assert!(user.starts_with("Conversation to summarize:"));
    }

    #[tokio::test]
    async fn model_summarizer_returns_model_text() {
        let model = Arc::new(ScriptedModel::final_text("NINE-SECTION-SUMMARY"));
        let s = ModelSummarizer::new(model.clone(), "m");
        let out = s
            .summarize(SummaryInput {
                transcript: "user: do the thing".into(),
                previous_summary: None,
            })
            .await
            .unwrap();
        assert_eq!(out, "NINE-SECTION-SUMMARY");
        assert_eq!(*model.calls.lock().unwrap(), 1);
    }
}
