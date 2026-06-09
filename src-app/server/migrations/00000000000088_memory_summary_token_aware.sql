-- Token-aware summarization replaces the message-COUNT trigger.
--
-- A message can be 5 tokens or 50K; counting messages is a loose proxy for the
-- thing we actually care about (context-window pressure). Switch the two
-- summarizer knobs to estimated-token thresholds (chars/4 heuristic, computed in
-- the summarizer). Dropping the count columns cascades their CHECK constraints.

ALTER TABLE memory_admin_settings
    DROP COLUMN IF EXISTS summarize_after_n_messages,
    DROP COLUMN IF EXISTS summarizer_keep_recent;

ALTER TABLE memory_admin_settings
    ADD COLUMN summarize_after_tokens INTEGER NOT NULL DEFAULT 12000
        CHECK (summarize_after_tokens >= 500 AND summarize_after_tokens <= 1000000),
    ADD COLUMN summarizer_keep_recent_tokens INTEGER NOT NULL DEFAULT 3000
        CHECK (summarizer_keep_recent_tokens >= 100);

-- keep-recent must stay below the trigger.
ALTER TABLE memory_admin_settings
    ADD CONSTRAINT summarizer_keep_lt_trigger
        CHECK (summarizer_keep_recent_tokens < summarize_after_tokens);
