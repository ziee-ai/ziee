//! Context compaction (Group I / LOCK-6) — a CORE (always-on) `AgentExtension`.
//!
//! Compaction is a core-loop responsibility, not a tool (Letta: "built into the
//! core loop"). The [`Compactor`] is a **tiered, window-relative, anti-thrash**
//! pipeline that runs cheapest-first and summarizes LAST (SOTA — Anthropic
//! context-editing + compaction, Claude Code's tiered pipeline, Cursor, Letta):
//!
//! 1. **Tier 1 — tool-result eviction** (no-LLM). Clears the CONTENT of OLD
//!    tool-result blocks (keeping the newest-N verbatim as the prompt-cache
//!    prefix), leaving a generic recall placeholder. tool_use/tool_result
//!    PAIRING is preserved (the block + its `tool_use_id` stay; only its content
//!    shrinks) so a provider never sees an orphan (→ 400). Fires at
//!    `tier1_fraction × usable` (~0.50). DEC-123/140.
//! 2. **Tier 2 — turn-trim** (no-LLM). Drops empty-ack / exact-duplicate pure
//!    text turns before summarizing; never drops a tool-bearing message (pairing
//!    safety) nor the newest message. DEC-123.
//! 3. **Tier 4 — summary** (the one LLM lever, LAST resort). Fires only if the
//!    cheap tiers didn't bring the request under the HIGH watermark. The
//!    old/keep split is measured in **TOKENS not message-count**, targeting the
//!    LOW watermark net of the summary's own size, with a **min-growth guard**
//!    (never pay for a summary that frees < `min_free_tokens`) and an optional
//!    **cooldown** hook (a caller-supplied "last-compaction token mark").
//!
//! **Window-relative trigger (DEC-121/122).** There is no fixed soft limit: the
//! usable window = `context_window − max_output − safety` (a conservative
//! `fallback_window_tokens` when `context_window` is `None`), and every tier's
//! threshold is a fraction of it. The server passes the fractions + tunables
//! (from `summarization_admin_settings`) as DATA — this crate is domain-free.
//!
//! **OUTBOUND-ONLY (DEC-138).** Every tier shapes only the request SENT to the
//! model. The cheap tiers (eviction/turn-trim) mutate `req.messages` and never
//! touch the transcript. Only the summary tier writes back via
//! `TranscriptStore::replace_head` + emits `AgentEvent::HistoryReplaced` — the
//! existing contract (kept intact; retiring that write is a separate
//! server-coordinated tranche, ITEM-56). Pinned `System` blocks, the newest-N,
//! and the verbatim latest user message are always kept.
//!
//! **Ordering.** [`CompactionExtension`] runs at a LATE `order`
//! ([`COMPACTION_ORDER`]) so — via the now-honored `.order()` sort
//! (`sorted_extensions`) — every other context contributor (assistant prompt,
//! params, memory, task-list re-injection) shapes the request FIRST. Within the
//! extension the tiers run cheapest→lossiest, so the expensive summary rarely
//! fires. The tokenizer is INJECTABLE ([`Compactor::with_token_counter`]); a
//! tokenizer-accurate count is a later server tranche (DEC-132).

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
/// contributors have shaped the request first (the tiers are ordered INSIDE the
/// extension: cheap no-LLM tiers before the summary tier).
pub const COMPACTION_ORDER: i32 = 1000;

/// Tokens reserved for the summary block itself when computing the token split,
/// so `pinned + summary + keep` lands at/under the low watermark by construction.
const SUMMARY_RESERVE_TOKENS: usize = 2000;

/// The generic placeholder left in place of an evicted tool-result's content.
/// Server-agnostic on purpose (DEC-134): the host maps "recall" onto its own
/// handle (chat → `get_tool_result`); this crate names no server-specific tool.
const TOOL_RESULT_PLACEHOLDER: &str =
    "[Older tool result cleared to save context. Recall it with get_tool_result if still needed.]";

/// A pluggable token counter (`text → tokens`). Defaults to [`estimate_tokens`]
/// (~chars/4); a server tranche can inject a tokenizer-accurate impl (DEC-132)
/// without changing the pipeline.
pub type TokenCounter = Arc<dyn Fn(&str) -> usize + Send + Sync>;

/// Window-relative + anti-thrash tunables (LOCK-6). All are DATA the host
/// supplies (chat/agent fractions come from `summarization_admin_settings`); the
/// [`Default`] is a conservative fallback, and [`chat`](Self::chat) /
/// [`agent`](Self::agent) are convenience presets (DEC-139) the server may
/// override field-by-field.
#[derive(Debug, Clone)]
pub struct CompactionConfig {
    /// The resolved per-model context window (tokens). `None` → use
    /// [`fallback_window_tokens`](Self::fallback_window_tokens) (DEC-122).
    pub context_window: Option<usize>,
    /// Reserved output headroom subtracted from the window (max output tokens).
    pub max_output_tokens: usize,
    /// Extra safety margin subtracted from the window.
    pub safety_tokens: usize,
    /// Conservative raw-window fallback when `context_window` is `None` (headroom
    /// is still subtracted from it, exactly like a real window).
    pub fallback_window_tokens: usize,
    /// HIGH watermark: the summary tier fires above `trigger_fraction × usable`.
    pub trigger_fraction: f64,
    /// LOW watermark: the summary tier compacts DOWN TO `low_watermark_fraction ×
    /// usable` (the ~20% deadband below `trigger_fraction` is the anti-thrash).
    pub low_watermark_fraction: f64,
    /// The cheap no-LLM tiers (eviction/turn-trim) fire above `tier1_fraction ×
    /// usable` (~0.50, below the summary trigger).
    pub tier1_fraction: f64,
    /// Newest tool-result messages kept verbatim by Tier 1 (cache-prefix; DEC-140).
    pub keep_tool_results: usize,
    /// Min tokens a summary must free, else it is skipped (min-growth guard —
    /// a single large message can't cause back-to-back compactions; DEC-125).
    pub min_free_tokens: usize,
    /// Cooldown growth: when a caller passes a "last-compaction token mark", the
    /// summary tier won't re-fire until the context has grown by at least this
    /// much since that mark (DEC-126). The high/low deadband is the primary
    /// anti-thrash; this is the hook for durable per-surface state (server tranche).
    pub cooldown_growth_tokens: usize,
}

impl Default for CompactionConfig {
    fn default() -> Self {
        // DEC-122/125/126/139/140 defaults (chat posture; agent overrides fractions).
        Self {
            context_window: None,
            max_output_tokens: 8_000,
            safety_tokens: 4_000,
            fallback_window_tokens: 128_000,
            trigger_fraction: 0.60,
            low_watermark_fraction: 0.40,
            tier1_fraction: 0.50,
            keep_tool_results: 6,
            min_free_tokens: 20_000,
            cooldown_growth_tokens: 10_000,
        }
    }
}

impl CompactionConfig {
    /// Chat posture — eager (high 0.60 → low 0.40). DEC-139.
    pub fn chat() -> Self {
        Self {
            trigger_fraction: 0.60,
            low_watermark_fraction: 0.40,
            ..Self::default()
        }
    }

    /// Agent posture — patient (high 0.75 → low 0.55). DEC-139.
    pub fn agent() -> Self {
        Self {
            trigger_fraction: 0.75,
            low_watermark_fraction: 0.55,
            ..Self::default()
        }
    }
}

/// The outcome of a compaction pass that actually fired.
#[derive(Clone)]
pub struct CompactionResult {
    /// The rewritten OUTBOUND message list (pinned + [summary] + kept newest, or
    /// just the evicted/trimmed list when only the cheap tiers fired).
    pub messages: Vec<ChatMessage>,
    /// Set ONLY when the summary tier fired: `(summary block, upto)` for
    /// `replace_head`. `None` when only the cheap no-LLM tiers fired — those are
    /// OUTBOUND-ONLY and never touch the transcript head (DEC-138).
    pub summary_write: Option<(ChatMessage, usize)>,
}

/// The tiered, window-relative summarizer. Holds a `ModelClient` for the summary
/// call + a [`CompactionConfig`] + an injectable [`TokenCounter`]. `fit` is pure
/// w.r.t. side effects (no transcript / sink), so it is directly unit-testable
/// with a fake model.
pub struct Compactor {
    model: Arc<dyn ModelClient>,
    /// Model name written into the summary `ChatRequest`.
    pub model_name: String,
    /// Window-relative + anti-thrash tunables (server-supplied DATA).
    pub config: CompactionConfig,
    /// Injectable token counter (defaults to `estimate_tokens`; DEC-132).
    count: TokenCounter,
}

impl Compactor {
    pub fn new(
        model: Arc<dyn ModelClient>,
        model_name: impl Into<String>,
        config: CompactionConfig,
    ) -> Self {
        Self {
            model,
            model_name: model_name.into(),
            config,
            count: Arc::new(estimate_tokens),
        }
    }

    /// Swap the token counter (a server tranche injects a tokenizer-accurate one).
    pub fn with_token_counter(mut self, counter: TokenCounter) -> Self {
        self.count = counter;
        self
    }

    // -- window-relative thresholds (window − headroom, × fraction) -----------

    /// The usable window in tokens: `context_window − max_output − safety`
    /// (falling back to `fallback_window_tokens` when the window is unknown).
    /// Never 0 (clamped to ≥ 1) so a threshold is always meaningful (DEC-122).
    pub fn usable_window(&self) -> usize {
        let raw = self
            .config
            .context_window
            .unwrap_or(self.config.fallback_window_tokens);
        raw.saturating_sub(self.config.max_output_tokens + self.config.safety_tokens)
            .max(1)
    }

    fn threshold(&self, fraction: f64) -> usize {
        ((self.usable_window() as f64) * fraction).floor() as usize
    }

    /// HIGH watermark (the summary tier fires above this).
    pub fn high_watermark_tokens(&self) -> usize {
        self.threshold(self.config.trigger_fraction)
    }

    /// LOW watermark (the summary tier compacts down to this).
    pub fn low_watermark_tokens(&self) -> usize {
        self.threshold(self.config.low_watermark_fraction)
    }

    /// The cheap no-LLM tiers fire above this.
    pub fn tier1_tokens(&self) -> usize {
        self.threshold(self.config.tier1_fraction)
    }

    // -- token estimation ------------------------------------------------------

    /// Flatten a message to its text for token estimation (tool_use → `name
    /// input`; tool_result → its text blocks; thinking → its text).
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

    fn estimate_msg(&self, m: &ChatMessage) -> usize {
        (self.count)(&Self::message_text(m))
    }

    /// Estimate the token cost of a message list (via the injected counter).
    pub fn estimate(&self, messages: &[ChatMessage]) -> usize {
        messages.iter().map(|m| self.estimate_msg(m)).sum()
    }

    // -- Tier 1: tool-result eviction (no-LLM, pairing-safe) -------------------

    /// Clear the CONTENT of OLD tool-result blocks (all but the newest
    /// `keep_tool_results`), leaving a recall placeholder. Excludes pinned
    /// `System` messages + the newest-N tool results (cache-prefix; DEC-140).
    /// Message COUNT is unchanged and each `tool_use_id` is preserved, so
    /// tool_use/tool_result pairing is intact. Returns `(list, changed)`.
    fn evict_tool_results(&self, msgs: &[ChatMessage]) -> (Vec<ChatMessage>, bool) {
        let tr_indices: Vec<usize> = msgs
            .iter()
            .enumerate()
            .filter(|(_, m)| {
                m.role != Role::System
                    && m.content
                        .iter()
                        .any(|b| matches!(b, ContentBlock::ToolResult { .. }))
            })
            .map(|(i, _)| i)
            .collect();

        // Keep the newest-N tool-result messages verbatim; evict the older ones.
        let evict_upto = tr_indices.len().saturating_sub(self.config.keep_tool_results);
        let evict_set: std::collections::HashSet<usize> =
            tr_indices[..evict_upto].iter().copied().collect();

        if evict_set.is_empty() {
            return (msgs.to_vec(), false);
        }

        let mut changed = false;
        let out = msgs
            .iter()
            .enumerate()
            .map(|(i, m)| {
                if !evict_set.contains(&i) {
                    return m.clone();
                }
                let new_content = m
                    .content
                    .iter()
                    .map(|b| match b {
                        ContentBlock::ToolResult {
                            tool_use_id,
                            name,
                            is_error,
                            content,
                        } => {
                            let orig_len: usize = content
                                .iter()
                                .filter_map(|c| match c {
                                    ContentBlock::Text { text } => Some(text.len()),
                                    _ => None,
                                })
                                .sum();
                            // Only rewrite when it actually shrinks the block.
                            if orig_len > TOOL_RESULT_PLACEHOLDER.len() {
                                changed = true;
                                ContentBlock::ToolResult {
                                    tool_use_id: tool_use_id.clone(),
                                    name: name.clone(),
                                    is_error: *is_error,
                                    content: vec![ContentBlock::Text {
                                        text: TOOL_RESULT_PLACEHOLDER.to_string(),
                                    }],
                                }
                            } else {
                                b.clone()
                            }
                        }
                        other => other.clone(),
                    })
                    .collect();
                ChatMessage {
                    role: m.role,
                    content: new_content,
                }
            })
            .collect();
        (out, changed)
    }

    // -- Tier 2: turn-trim (no-LLM, pairing-safe) ------------------------------

    /// Drop empty-ack + exact-duplicate PURE-TEXT turns. Never drops a `System`
    /// message, the newest message, or any tool-bearing message (so a
    /// tool_result is never orphaned; DEC-123). Returns `(list, changed)`.
    fn turn_trim(&self, msgs: &[ChatMessage]) -> (Vec<ChatMessage>, bool) {
        if msgs.len() <= 1 {
            return (msgs.to_vec(), false);
        }
        let last = msgs.len() - 1;
        let mut out: Vec<ChatMessage> = Vec::with_capacity(msgs.len());
        let mut changed = false;
        for (i, m) in msgs.iter().enumerate() {
            let has_tools = msg_has_tool_block(m);
            let trimmable = m.role != Role::System && !has_tools && i != last;
            if trimmable {
                let text = Self::message_text(m);
                let text = text.trim();
                // Empty ack (no substantive text, no tool blocks).
                if text.is_empty() {
                    changed = true;
                    continue;
                }
                // Exact duplicate of the previous kept pure-text same-role turn.
                let dup = out.last().is_some_and(|p| {
                    p.role == m.role
                        && !msg_has_tool_block(p)
                        && Self::message_text(p).trim() == text
                });
                if dup {
                    changed = true;
                    continue;
                }
            }
            out.push(m.clone());
        }
        (out, changed)
    }

    // -- Tier 4: token-split for the summary (pairing-safe) --------------------

    /// The index into `rest` at which the KEPT newest suffix begins — the largest
    /// newest suffix whose tokens ≤ `target_keep` (always keeps ≥ the newest
    /// message). `rest[..keep_start]` is summarized; `rest[keep_start..]` is kept.
    fn split_keep_start(&self, rest: &[ChatMessage], target_keep: usize) -> usize {
        let mut acc = 0usize;
        let mut keep_start = rest.len();
        for i in (0..rest.len()).rev() {
            let t = self.estimate_msg(&rest[i]);
            // Always keep the newest message; then add older while under target.
            if keep_start != rest.len() && acc + t > target_keep {
                break;
            }
            acc += t;
            keep_start = i;
        }
        keep_start
    }

    // -- the pipeline ----------------------------------------------------------

    /// Run the tiered pipeline. Returns `None` when nothing fired (under the
    /// cheapest tier's fire point, or no worthwhile compaction), else the
    /// rewritten OUTBOUND messages (+ a summary write only when the summary tier
    /// fired). `last_compaction_mark` is an optional anti-thrash hook (the token
    /// count at the previous compaction); pass `None` when no durable state.
    pub async fn fit(
        &self,
        messages: &[ChatMessage],
        last_compaction_mark: Option<usize>,
    ) -> Result<Option<CompactionResult>, AppError> {
        let tier1_threshold = self.tier1_tokens();
        let trigger_threshold = self.high_watermark_tokens();
        let low_watermark = self.low_watermark_tokens();

        let current = self.estimate(messages);
        if current <= tier1_threshold {
            // Below the cheapest tier's fire point — nothing to do (deadband).
            return Ok(None);
        }

        // Phase A — cheap, no-LLM, OUTBOUND-ONLY tiers (Tier 1 then Tier 2).
        let (evicted, did_evict) = self.evict_tool_results(messages);
        let (work, did_trim) = self.turn_trim(&evicted);
        let cheap_changed = did_evict || did_trim;
        let after_cheap = self.estimate(&work);

        // A helper: the cheap-tier outcome (used at every "don't summarize" exit).
        let cheap_result = |changed: bool, work: Vec<ChatMessage>| {
            if changed {
                Some(CompactionResult {
                    messages: work,
                    summary_write: None,
                })
            } else {
                None
            }
        };

        if after_cheap <= trigger_threshold {
            // Cheap tiers brought us under the HIGH watermark → NO summary model
            // call (TEST-224). Outbound-only when they changed anything.
            return Ok(cheap_result(cheap_changed, work));
        }

        // Cooldown hook: if a caller-supplied mark says we compacted recently and
        // haven't grown by the cooldown amount, don't re-summarize this turn.
        if let Some(mark) = last_compaction_mark {
            if current.saturating_sub(mark) < self.config.cooldown_growth_tokens {
                return Ok(cheap_result(cheap_changed, work));
            }
        }

        // Phase B — summary tier (LAST resort). Split the ORIGINAL messages by
        // TOKENS so `upto` maps cleanly to the persisted transcript head and the
        // kept newest lands at/under the low watermark net of the summary.
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
        if rest.len() < 2 {
            // Nothing meaningful to summarize (all pinned, or a single turn).
            return Ok(cheap_result(cheap_changed, work));
        }

        let pinned_tokens = self.estimate(&pinned);
        let summary_reserve = SUMMARY_RESERVE_TOKENS.min(low_watermark / 2);
        let target_keep = low_watermark
            .saturating_sub(pinned_tokens + summary_reserve)
            .max(1);

        let mut keep_start = self.split_keep_start(&rest, target_keep);
        // Pairing: the kept suffix must not START with a tool_result whose
        // tool_use would be summarized away (→ provider 400). Pull the boundary
        // earlier to bring the tool_use into `keep` with its result.
        while keep_start > 0 && msg_has_tool_result(&rest[keep_start]) {
            keep_start -= 1;
        }
        if keep_start == 0 {
            // Everything ends up kept — nothing older to summarize.
            return Ok(cheap_result(cheap_changed, work));
        }
        let old = &rest[..keep_start];
        let keep = &rest[keep_start..];

        // Min-growth guard: only pay for a summary that frees a worthwhile amount
        // (a single large recent message can't force back-to-back compactions).
        if self.estimate(old) < self.config.min_free_tokens {
            return Ok(cheap_result(cheap_changed, work));
        }

        // Summarize `old` via the model. Freeform prompt for now — the structured
        // 9-section template (ITEM-60) is a separate server-coordinated tranche.
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

        let upto = keep_start;
        let summary = ChatMessage::system(format!(
            "[Summary of {upto} earlier messages]\n{summary_text}"
        ));

        let mut messages_out = pinned;
        messages_out.push(summary.clone());
        messages_out.extend_from_slice(keep);

        Ok(Some(CompactionResult {
            messages: messages_out,
            summary_write: Some((summary, upto)),
        }))
    }
}

/// Does this message carry any tool_use OR tool_result block?
fn msg_has_tool_block(m: &ChatMessage) -> bool {
    m.content.iter().any(|b| {
        matches!(
            b,
            ContentBlock::ToolUse { .. } | ContentBlock::ToolResult { .. }
        )
    })
}

/// Does this message carry a tool_result block?
fn msg_has_tool_result(m: &ChatMessage) -> bool {
    m.content
        .iter()
        .any(|b| matches!(b, ContentBlock::ToolResult { .. }))
}

/// The core extension that runs the [`Compactor`] pipeline in the loop and wires
/// its side effects. OUTBOUND-ONLY: the cheap tiers mutate only `req.messages`;
/// only the summary tier persists via `replace_head` + emits `HistoryReplaced`.
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
        // Durable per-surface cooldown state is a server tranche; pass `None`.
        if let Some(res) = self.compactor.fit(&req.messages, None).await? {
            req.messages = res.messages;
            // Only the summary tier touches the transcript head (OUTBOUND-ONLY:
            // the cheap eviction/turn-trim tiers leave `summary_write == None`).
            if let Some((summary, upto)) = res.summary_write {
                self.transcript
                    .replace_head(self.run_id, summary, upto)
                    .await?;
                self.sink
                    .emit(AgentEvent::HistoryReplaced { summary_upto: upto })
                    .await;
            }
        }
        Ok(Flow::Continue)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_fakes::{assistant_tool, FakeSink, FakeTranscript, ScriptedModel};

    // ~120 chars → ~30 tokens each under the chars/4 estimator.
    fn big_user(tag: usize) -> ChatMessage {
        ChatMessage::user(format!(
            "message {tag}: {}",
            "lorem ipsum dolor sit amet ".repeat(4)
        ))
    }

    /// A user tool_result message carrying `n_chars` of content (the paired
    /// tool_use is `assistant_tool` with the same id).
    fn tool_result_msg(id: &str, n_chars: usize) -> ChatMessage {
        ChatMessage::with_blocks(
            Role::User,
            vec![ContentBlock::ToolResult {
                tool_use_id: id.to_string(),
                name: Some("search".into()),
                content: vec![ContentBlock::Text {
                    text: "x".repeat(n_chars),
                }],
                is_error: None,
            }],
        )
    }

    /// A small window config so unit-token-sized messages exercise the pipeline.
    /// usable = 1000 − 100 − 0 = 900 → tier1 450, trigger 540, low 360.
    fn small_cfg() -> CompactionConfig {
        CompactionConfig {
            context_window: Some(1000),
            max_output_tokens: 100,
            safety_tokens: 0,
            keep_tool_results: 2,
            min_free_tokens: 50,
            ..CompactionConfig::default()
        }
    }

    // ---- window-relative trigger (TEST-234 / TEST-235 / DEC-121/122) ----------

    #[test]
    fn trigger_scales_with_context_window() {
        let model = Arc::new(ScriptedModel::final_text("s"));
        let big = Compactor::new(
            model.clone(),
            "m",
            CompactionConfig {
                context_window: Some(400_000),
                ..CompactionConfig::chat()
            },
        );
        let small = Compactor::new(
            model,
            "m",
            CompactionConfig {
                context_window: Some(40_000),
                ..CompactionConfig::chat()
            },
        );
        // usable = window − (8000 + 4000); high watermark = 0.60 × usable.
        assert_eq!(big.usable_window(), 400_000 - 12_000);
        assert_eq!(big.high_watermark_tokens(), ((388_000f64) * 0.60) as usize);
        assert_eq!(small.usable_window(), 40_000 - 12_000);
        assert_eq!(small.high_watermark_tokens(), ((28_000f64) * 0.60) as usize);
        // The trigger scales DOWN with the window.
        assert!(small.high_watermark_tokens() < big.high_watermark_tokens());
    }

    #[test]
    fn trigger_falls_back_when_window_none() {
        let model = Arc::new(ScriptedModel::final_text("s"));
        // chat defaults, no context_window → fallback 128000.
        let c = Compactor::new(model, "m", CompactionConfig::chat());
        // usable = 128000 − 8000 − 4000 = 116000; trigger 0.60 → 69600.
        assert_eq!(c.usable_window(), 116_000);
        assert_eq!(c.high_watermark_tokens(), 69_600);
        assert_eq!(c.low_watermark_tokens(), 46_400);
        // agent posture: patient 0.75 → 87000.
        let a = Compactor::new(
            Arc::new(ScriptedModel::final_text("s")),
            "m",
            CompactionConfig::agent(),
        );
        assert_eq!(a.high_watermark_tokens(), 87_000);
    }

    // ---- Tier 1 eviction excludes pinned + newest-N (TEST-225 / DEC-140) ------

    #[test]
    fn eviction_excludes_pinned_and_newest_n() {
        let c = Compactor::new(Arc::new(ScriptedModel::final_text("s")), "m", small_cfg());
        // System pinned + 4 tool results (keep_tool_results = 2).
        let msgs = vec![
            ChatMessage::system("PINNED-CORE-MEMORY"),
            tool_result_msg("t1", 400),
            tool_result_msg("t2", 400),
            tool_result_msg("t3", 400),
            tool_result_msg("t4", 400),
        ];
        let (out, changed) = c.evict_tool_results(&msgs);
        assert!(changed);
        // Pinned System block untouched.
        assert!(Compactor::message_text(&out[0]).contains("PINNED-CORE-MEMORY"));
        // Oldest two (t1, t2) evicted → placeholder; newest two (t3, t4) verbatim.
        assert!(Compactor::message_text(&out[1]).contains("Recall it with get_tool_result"));
        assert!(Compactor::message_text(&out[2]).contains("Recall it with get_tool_result"));
        assert_eq!(Compactor::message_text(&out[3]).trim(), "x".repeat(400));
        assert_eq!(Compactor::message_text(&out[4]).trim(), "x".repeat(400));
        // tool_use_id preserved on every evicted block (pairing intact).
        for m in &out[1..] {
            assert!(m
                .content
                .iter()
                .any(|b| matches!(b, ContentBlock::ToolResult { .. })));
        }
    }

    // ---- eviction BEFORE the summary tier (TEST-224 / DEC-123) ----------------

    #[tokio::test]
    async fn eviction_avoids_summary_model_call() {
        let model = Arc::new(ScriptedModel::final_text("SHOULD-NOT-BE-CALLED"));
        let c = Compactor::new(model.clone(), "m", small_cfg());
        // 6 tool-result pairs of ~125 tokens each (~750 tokens) → over trigger 540.
        let mut msgs = vec![ChatMessage::user("do the research task")];
        for i in 0..6 {
            let id = format!("t{i}");
            msgs.push(assistant_tool(&id, "search", serde_json::json!({})));
            msgs.push(tool_result_msg(&id, 500));
        }
        assert!(c.estimate(&msgs) > c.high_watermark_tokens());

        let res = c
            .fit(&msgs, None)
            .await
            .unwrap()
            .expect("cheap tiers fired");
        // The summary model was NEVER called…
        assert_eq!(*model.calls.lock().unwrap(), 0);
        // …and no transcript write was produced (cheap tiers are outbound-only).
        assert!(res.summary_write.is_none());
        // Eviction dropped it under the high watermark.
        assert!(c.estimate(&res.messages) <= c.high_watermark_tokens());
    }

    // ---- turn-trim never orphans a tool_result (TEST-227 / DEC-123) -----------

    #[test]
    fn turn_trim_preserves_tool_pairing() {
        let c = Compactor::new(Arc::new(ScriptedModel::final_text("s")), "m", small_cfg());
        let msgs = vec![
            ChatMessage::user("start"),
            assistant_tool("tp", "search", serde_json::json!({})),
            tool_result_msg("tp", 40),
            ChatMessage::assistant("   "), // empty ack → droppable
            ChatMessage::user("final question"),
        ];
        let (out, changed) = c.turn_trim(&msgs);
        assert!(changed);
        // The empty ack is gone.
        assert!(!out
            .iter()
            .any(|m| Compactor::message_text(m).trim().is_empty()));
        // Every kept tool_result still has a matching tool_use id (no orphan).
        for m in &out {
            for b in &m.content {
                if let ContentBlock::ToolResult { tool_use_id, .. } = b {
                    let id = tool_use_id.clone();
                    assert!(out.iter().any(|mm| mm.content.iter().any(|bb| matches!(
                        bb,
                        ContentBlock::ToolUse { id: uid, .. } if *uid == id
                    ))));
                }
            }
        }
    }

    // ---- summary split by TOKENS lands ≤ low watermark (TEST-243 / DEC-124) ----

    #[tokio::test]
    async fn summary_split_lands_at_low_watermark() {
        let model = Arc::new(ScriptedModel::final_text("SUMMARY-TEXT"));
        let c = Compactor::new(model.clone(), "m", small_cfg());
        // 20 plain turns (~30 tokens each ≈ 600 tokens) → over trigger 540, and no
        // tool results / acks so the cheap tiers are no-ops → the summary tier runs.
        let msgs: Vec<ChatMessage> = (0..20).map(big_user).collect();
        assert!(c.estimate(&msgs) > c.high_watermark_tokens());

        let res = c
            .fit(&msgs, None)
            .await
            .unwrap()
            .expect("summary tier fired");
        // The summary tier fired (model called once) + wrote back.
        assert_eq!(*model.calls.lock().unwrap(), 1);
        let (summary, upto) = res.summary_write.clone().expect("summary write");
        assert!(Compactor::message_text(&summary).contains("SUMMARY-TEXT"));
        assert!(upto >= 1);
        // Split by tokens → the rewritten request is at/under the LOW watermark.
        assert!(c.estimate(&res.messages) <= c.low_watermark_tokens());
    }

    // ---- anti-thrash: N normal additions after a compaction don't re-fire
    //      (TEST-241 / DEC-124/125) ------------------------------------------

    #[tokio::test]
    async fn no_retrigger_after_compaction() {
        let model = Arc::new(ScriptedModel::final_text("SUM"));
        let c = Compactor::new(model.clone(), "m", small_cfg());
        // A post-compaction state: a summary block + a few kept turns ≈ low
        // watermark (~360). tier1 450 / trigger 540.
        let mut msgs = vec![ChatMessage::system("[Summary of 14 earlier messages]\nrecap")];
        for i in 0..6 {
            msgs.push(big_user(i)); // ~30 tokens each → ~180 + summary ≈ low band
        }
        // Append 4 NORMAL messages (~120 tokens) → ~300–480 total, still < trigger.
        for i in 100..104 {
            msgs.push(big_user(i));
        }
        assert!(c.estimate(&msgs) < c.high_watermark_tokens());

        // No tier fires (under trigger; nothing cheap to change) and the model is
        // never called.
        let res = c.fit(&msgs, None).await.unwrap();
        assert!(res.is_none());
        assert_eq!(*model.calls.lock().unwrap(), 0);
    }

    // ---- min-growth guard blocks a summary (TEST-242 / DEC-125) --------------

    #[tokio::test]
    async fn min_growth_guard_blocks_summary() {
        let model = Arc::new(ScriptedModel::final_text("SUM"));
        // Small window but a LARGE min_free so the compactable `old` can't clear it.
        let cfg = CompactionConfig {
            min_free_tokens: 10_000,
            ..small_cfg()
        };
        let c = Compactor::new(model.clone(), "m", cfg);
        // Over trigger, but with no tool results/acks and a modest old portion:
        // the oldest compactable tokens are far below 10_000 → summary blocked.
        let msgs: Vec<ChatMessage> = (0..20).map(big_user).collect();
        assert!(c.estimate(&msgs) > c.high_watermark_tokens());

        let res = c.fit(&msgs, None).await.unwrap();
        // No summary written, model never called (min-growth guard held).
        assert!(res.is_none() || res.as_ref().unwrap().summary_write.is_none());
        assert_eq!(*model.calls.lock().unwrap(), 0);
    }

    // ---- cooldown-mark hook blocks a re-summary (DEC-126) --------------------

    #[tokio::test]
    async fn cooldown_mark_blocks_resummary() {
        let model = Arc::new(ScriptedModel::final_text("SUM"));
        let c = Compactor::new(model.clone(), "m", small_cfg());
        let msgs: Vec<ChatMessage> = (0..20).map(big_user).collect();
        let current = c.estimate(&msgs);
        // A "last compaction" mark just below current → growth < cooldown (10_000).
        let res = c.fit(&msgs, Some(current.saturating_sub(10))).await.unwrap();
        assert!(res.is_none() || res.as_ref().unwrap().summary_write.is_none());
        assert_eq!(*model.calls.lock().unwrap(), 0);
        // With NO mark the same input DOES summarize (proves the mark is the gate).
        let res2 = c.fit(&msgs, None).await.unwrap().expect("summary");
        assert!(res2.summary_write.is_some());
        assert_eq!(*model.calls.lock().unwrap(), 1);
    }

    // ---- extension side effects (outbound-only for cheap; head-write for summary) --

    #[tokio::test]
    async fn extension_summary_replaces_head_and_emits() {
        let model = Arc::new(ScriptedModel::final_text("SUMMARY"));
        let compactor = Compactor::new(model, "m", small_cfg());
        let transcript = Arc::new(FakeTranscript::default());
        let sink = Arc::new(FakeSink::default());
        let run_id = Uuid::new_v4();
        let ext = CompactionExtension::new(compactor, transcript.clone(), sink.clone(), run_id);

        let messages: Vec<ChatMessage> = (0..20).map(big_user).collect();
        let before = messages.len();
        let mut req = ChatRequest {
            model: "m".into(),
            messages,
            ..Default::default()
        };
        let flow = ext.before_model(&mut req).await.unwrap();
        assert_eq!(flow, Flow::Continue);
        assert!(req.messages.len() < before);
        // The summary tier wrote back + emitted HistoryReplaced.
        assert_eq!(transcript.replaced.lock().unwrap().len(), 1);
        assert_eq!(transcript.replaced.lock().unwrap()[0].0, run_id);
        assert!(sink
            .events
            .lock()
            .unwrap()
            .iter()
            .any(|e| matches!(e, AgentEvent::HistoryReplaced { .. })));
    }

    #[tokio::test]
    async fn extension_cheap_tier_is_outbound_only() {
        // Eviction alone reduces the request → NO head write / HistoryReplaced.
        let model = Arc::new(ScriptedModel::final_text("UNUSED"));
        let compactor = Compactor::new(model.clone(), "m", small_cfg());
        let transcript = Arc::new(FakeTranscript::default());
        let sink = Arc::new(FakeSink::default());
        let ext = CompactionExtension::new(
            compactor,
            transcript.clone(),
            sink.clone(),
            Uuid::new_v4(),
        );

        let mut msgs = vec![ChatMessage::user("task")];
        for i in 0..6 {
            let id = format!("t{i}");
            msgs.push(assistant_tool(&id, "search", serde_json::json!({})));
            msgs.push(tool_result_msg(&id, 500));
        }
        let mut req = ChatRequest {
            model: "m".into(),
            messages: msgs,
            ..Default::default()
        };
        ext.before_model(&mut req).await.unwrap();
        // Outbound request shrank, but the transcript head was NOT rewritten.
        assert_eq!(*model.calls.lock().unwrap(), 0);
        assert!(transcript.replaced.lock().unwrap().is_empty());
        assert!(sink.events.lock().unwrap().is_empty());
    }

    #[test]
    fn extension_is_core_and_late() {
        let model = Arc::new(ScriptedModel::final_text("x"));
        let ext = CompactionExtension::new(
            Compactor::new(model, "m", CompactionConfig::default()),
            Arc::new(FakeTranscript::default()),
            Arc::new(FakeSink::default()),
            Uuid::new_v4(),
        );
        assert!(ext.is_core());
        assert_eq!(ext.order(), COMPACTION_ORDER);
    }
}
