//! Context compaction (ITEM-6, P3) — a CORE (always-on) `AgentExtension`.
//!
//! Compaction is a core-loop responsibility, not a tool (Letta: "built into the
//! core loop"). The [`Compactor`] implements the sliding window: when the wire
//! messages exceed a window-relative soft limit, summarize the oldest ~30% via
//! the model into a single system block and keep the newest ~70% verbatim.
//! Pinned (`System`) blocks — core memory — are kept verbatim, never summarized.
//!
//! [`CompactionExtension`] runs this in the loop via `before_model` at a LATE
//! `order`, persists the summary via `TranscriptStore::replace_head`, and emits
//! `AgentEvent::HistoryReplaced` so the host re-syncs its cache (Goose's
//! `HistoryReplaced`). The loop itself has NO bespoke `compactor.fit()` call.

use std::sync::Arc;

use ai_providers::{ChatMessage, ChatRequest, ContentBlock, Role};
use async_trait::async_trait;
use uuid::Uuid;
use ziee_core::AppError;

use crate::core::ModelClient;
use crate::extension::{AgentExtension, Flow};
use crate::ports::{EventSink, TranscriptStore};
use crate::tokens::estimate_tokens;
use crate::types::AgentEvent;

/// The order the compaction hook runs at — LATE, so all other `before_model`
/// contributors have shaped the request first.
pub const COMPACTION_ORDER: i32 = 1000;

/// The default fraction of non-pinned messages kept verbatim (Letta ~70%).
const DEFAULT_KEEP_FRACTION: f64 = 0.7;

/// The outcome of a compaction pass that actually fired.
#[derive(Debug, Clone)]
pub struct CompactionResult {
    /// The rewritten message list (pinned + summary + kept newest).
    pub messages: Vec<ChatMessage>,
    /// The summary system block written back into the transcript head.
    pub summary: ChatMessage,
    /// How many raw messages the summary stands in for (`replace_head` `upto`).
    pub upto: usize,
}

/// The sliding-window summarizer. Holds a `ModelClient` for the summary call;
/// `fit` is pure w.r.t. side effects (no transcript / sink), so it is directly
/// unit-testable with a fake model.
pub struct Compactor {
    model: Arc<dyn ModelClient>,
    /// Model name written into the summary `ChatRequest`.
    pub model_name: String,
    /// Window-relative soft limit (tokens) above which compaction fires.
    pub soft_limit_tokens: usize,
    /// Fraction of non-pinned messages kept verbatim (default 0.7).
    pub keep_fraction: f64,
}

impl Compactor {
    pub fn new(
        model: Arc<dyn ModelClient>,
        model_name: impl Into<String>,
        soft_limit_tokens: usize,
    ) -> Self {
        Self {
            model,
            model_name: model_name.into(),
            soft_limit_tokens,
            keep_fraction: DEFAULT_KEEP_FRACTION,
        }
    }

    fn message_text(m: &ChatMessage) -> String {
        m.content
            .iter()
            .filter_map(|b| match b {
                ContentBlock::Text { text } => Some(text.clone()),
                ContentBlock::Thinking { thinking, .. } => Some(thinking.clone()),
                ContentBlock::ToolUse { name, input, .. } => Some(format!("{name} {input}")),
                ContentBlock::ToolResult { content, .. } => Some(
                    content
                        .iter()
                        .filter_map(|c| match c {
                            ContentBlock::Text { text } => Some(text.as_str()),
                            _ => None,
                        })
                        .collect::<Vec<_>>()
                        .join(" "),
                ),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// Estimate the token cost of a message list (~chars/4 per message).
    pub fn estimate(messages: &[ChatMessage]) -> usize {
        messages
            .iter()
            .map(|m| estimate_tokens(&Self::message_text(m)))
            .sum()
    }

    /// Decide + perform compaction. Returns `None` when under budget (or nothing
    /// summarizable), else the rewritten messages + the summary block.
    pub async fn fit(
        &self,
        messages: &[ChatMessage],
    ) -> Result<Option<CompactionResult>, AppError> {
        if Self::estimate(messages) <= self.soft_limit_tokens {
            return Ok(None);
        }

        // Pinned (System / core-memory) blocks are kept verbatim; only the rest
        // is eligible for summarization.
        let pinned: Vec<ChatMessage> = messages
            .iter()
            .filter(|m| m.role == Role::System)
            .cloned()
            .collect();
        let rest: Vec<ChatMessage> = messages
            .iter()
            .filter(|m| m.role != Role::System)
            .cloned()
            .collect();
        if rest.is_empty() {
            // Everything is pinned — nothing to compact.
            return Ok(None);
        }

        let oldest_count = (((rest.len() as f64) * (1.0 - self.keep_fraction)).ceil() as usize)
            .clamp(1, rest.len());
        let (old, keep) = rest.split_at(oldest_count);

        let joined = old
            .iter()
            .map(Self::message_text)
            .collect::<Vec<_>>()
            .join("\n\n");
        let summary_req = ChatRequest {
            model: self.model_name.clone(),
            messages: vec![
                ChatMessage::system(
                    "Summarize the earlier conversation below concisely, preserving key facts, \
                     decisions, and open tasks. Reply with the summary only.",
                ),
                ChatMessage::user(joined),
            ],
            ..Default::default()
        };
        let (summary_msg, _usage) = self.model.call(summary_req).await?;
        let summary_text = summary_msg
            .content
            .iter()
            .filter_map(|b| match b {
                ContentBlock::Text { text } => Some(text.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("");

        let summary = ChatMessage::system(format!(
            "[Summary of {oldest_count} earlier messages]\n{summary_text}"
        ));

        let mut new_messages = pinned;
        new_messages.push(summary.clone());
        new_messages.extend_from_slice(keep);

        Ok(Some(CompactionResult {
            messages: new_messages,
            summary,
            upto: oldest_count,
        }))
    }
}

/// The core extension that runs [`Compactor::fit`] in the loop and wires its
/// side effects (persist summary + emit `HistoryReplaced`).
pub struct CompactionExtension {
    compactor: Compactor,
    transcript: Arc<dyn TranscriptStore>,
    sink: Arc<dyn EventSink>,
    run_id: Uuid,
    order: i32,
}

impl CompactionExtension {
    pub fn new(
        compactor: Compactor,
        transcript: Arc<dyn TranscriptStore>,
        sink: Arc<dyn EventSink>,
        run_id: Uuid,
    ) -> Self {
        Self {
            compactor,
            transcript,
            sink,
            run_id,
            order: COMPACTION_ORDER,
        }
    }
}

#[async_trait]
impl AgentExtension for CompactionExtension {
    fn name(&self) -> &str {
        "compaction"
    }

    fn order(&self) -> i32 {
        self.order
    }

    fn is_core(&self) -> bool {
        true
    }

    async fn before_model(&self, req: &mut ChatRequest) -> Result<Flow, AppError> {
        if let Some(res) = self.compactor.fit(&req.messages).await? {
            req.messages = res.messages;
            self.transcript
                .replace_head(self.run_id, res.summary, res.upto)
                .await?;
            self.sink
                .emit(AgentEvent::HistoryReplaced {
                    summary_upto: res.upto,
                })
                .await;
        }
        Ok(Flow::Continue)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_fakes::{FakeSink, FakeTranscript, ScriptedModel};

    fn big_user(tag: usize) -> ChatMessage {
        // ~120 chars → ~30 tokens each.
        ChatMessage::user(format!(
            "message {tag}: {}",
            "lorem ipsum dolor sit amet ".repeat(4)
        ))
    }

    #[tokio::test]
    async fn fit_under_budget_returns_none() {
        let model = Arc::new(ScriptedModel::final_text("summary"));
        let compactor = Compactor::new(model, "m", 100_000);
        let messages = vec![big_user(1), big_user(2)];
        assert!(compactor.fit(&messages).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn fit_over_budget_summarizes_and_keeps_system() {
        let model = Arc::new(ScriptedModel::final_text("SUMMARY-TEXT"));
        let compactor = Compactor::new(model, "m", 20);
        let mut messages = vec![ChatMessage::system("PINNED-CORE-MEMORY")];
        for i in 0..10 {
            messages.push(big_user(i));
        }

        let res = compactor.fit(&messages).await.unwrap().expect("compacted");
        // Pinned system block survives verbatim.
        assert!(res
            .messages
            .iter()
            .any(|m| Compactor::message_text(m).contains("PINNED-CORE-MEMORY")));
        // The summary block is present and carries the model's summary text.
        assert!(Compactor::message_text(&res.summary).contains("SUMMARY-TEXT"));
        // Compaction shrank the list, and `upto` reflects the summarized count.
        assert!(res.messages.len() < messages.len());
        assert!(res.upto >= 1);
    }

    #[tokio::test]
    async fn extension_replaces_head_and_emits() {
        let model = Arc::new(ScriptedModel::final_text("SUMMARY"));
        let compactor = Compactor::new(model, "m", 20);
        let transcript = Arc::new(FakeTranscript::default());
        let sink = Arc::new(FakeSink::default());
        let run_id = Uuid::new_v4();
        let ext = CompactionExtension::new(
            compactor,
            transcript.clone(),
            sink.clone(),
            run_id,
        );

        let mut messages = vec![ChatMessage::system("PINNED")];
        for i in 0..10 {
            messages.push(big_user(i));
        }
        let before = messages.len();
        let mut req = ChatRequest {
            model: "m".into(),
            messages,
            ..Default::default()
        };

        let flow = ext.before_model(&mut req).await.unwrap();
        assert_eq!(flow, Flow::Continue);
        // Request was compacted in place.
        assert!(req.messages.len() < before);
        // replace_head was called for this run, and HistoryReplaced was emitted.
        assert_eq!(transcript.replaced.lock().unwrap().len(), 1);
        assert_eq!(transcript.replaced.lock().unwrap()[0].0, run_id);
        assert!(sink
            .events
            .lock()
            .unwrap()
            .iter()
            .any(|e| matches!(e, AgentEvent::HistoryReplaced { .. })));
    }

    #[test]
    fn extension_is_core_and_late() {
        let model = Arc::new(ScriptedModel::final_text("x"));
        let ext = CompactionExtension::new(
            Compactor::new(model, "m", 10),
            Arc::new(FakeTranscript::default()),
            Arc::new(FakeSink::default()),
            Uuid::new_v4(),
        );
        assert!(ext.is_core());
        assert_eq!(ext.order(), COMPACTION_ORDER);
    }
}
