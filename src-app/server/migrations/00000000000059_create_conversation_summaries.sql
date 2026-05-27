-- Rolling per-branch summary of conversation history (Phase 6).
--
-- When a branch exceeds N messages the summarizer extension condenses
-- the older messages into a single text block, freeing context budget.
-- The summary replaces those older messages in
-- `convert_history_to_messages_with_extensions` when assembling the
-- LLM prompt.
CREATE TABLE conversation_summaries (
    branch_id           UUID PRIMARY KEY REFERENCES branches(id) ON DELETE CASCADE,
    summary_text        TEXT NOT NULL,
    -- The message_id at which the summary STOPS — messages strictly
    -- after this id are kept verbatim in the prompt.
    summarized_up_to_id UUID REFERENCES messages(id) ON DELETE SET NULL,
    message_count       INTEGER NOT NULL DEFAULT 0,
    model_used          TEXT,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_conversation_summaries_branch
    ON conversation_summaries(branch_id);
