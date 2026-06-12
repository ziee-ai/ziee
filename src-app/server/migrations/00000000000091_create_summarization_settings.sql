-- Extract conversation summarization into its own feature.
--
-- Before this migration, summarization lived under the memory module:
-- four columns on `memory_admin_settings` drove the trigger thresholds
-- and prompt overrides, and summarization was implicitly gated on
-- `memory_admin_settings.enabled`. That coupling was historical, not
-- principled — memory is privacy-sensitive cross-conversation personal
-- fact storage (opt-in, default OFF), whereas summarization is just
-- within-a-conversation context compaction that benefits every
-- deployment. A deployment correctly keeping per-user memory OFF lost
-- summarization entirely.
--
-- This migration creates a standalone settings surface for the
-- summarizer and a per-conversation override (mirroring memory_mode),
-- then drops the four summarizer columns from memory_admin_settings.
--
-- ============================================================================
-- WARNING — DATA LOSS for operators who customized summarizer settings:
--   the new summarization_admin_settings row is seeded with COMPILED
--   DEFAULTS, NOT with the prior memory_admin_settings values. Custom
--   thresholds / prompt overrides must be re-entered via the new
--   `/settings/summarization-admin` page after this migration runs.
-- ============================================================================
--
-- New behaviour:
--   - Default ON for every deployment (zero-config compaction).
--   - When `default_summarization_model_id` is NULL, the chat extension
--     falls back to the conversation's own model — so summarization
--     works out of the box without an admin picking a model.
--   - Per-conversation override `inherit` / `on` / `off` (mirroring
--     conversation_memory_settings.memory_mode from migration 76).

-- ─── (a) Singleton admin settings ──────────────────────────────────────
CREATE TABLE summarization_admin_settings (
    id                              SMALLINT PRIMARY KEY DEFAULT 1
        CHECK (id = 1),
    enabled                         BOOLEAN NOT NULL DEFAULT TRUE,
    default_summarization_model_id  UUID REFERENCES llm_models(id) ON DELETE SET NULL,
    summarize_after_tokens          INTEGER NOT NULL DEFAULT 12000
        CHECK (summarize_after_tokens BETWEEN 500 AND 1000000),
    summarizer_keep_recent_tokens   INTEGER NOT NULL DEFAULT 3000
        CHECK (summarizer_keep_recent_tokens >= 100),
    full_summary_prompt             TEXT,
    incremental_summary_prompt      TEXT,
    updated_at                      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT summarizer_keep_lt_trigger
        CHECK (summarizer_keep_recent_tokens < summarize_after_tokens)
);

INSERT INTO summarization_admin_settings (id) VALUES (1) ON CONFLICT (id) DO NOTHING;

-- ─── (b) Per-conversation override ─────────────────────────────────────
-- Mirrors conversation_memory_settings (migration 76).
CREATE TABLE conversation_summarization_settings (
    conversation_id     UUID PRIMARY KEY REFERENCES conversations(id) ON DELETE CASCADE,
    summarization_mode  TEXT NOT NULL DEFAULT 'inherit'
        CHECK (summarization_mode IN ('inherit', 'on', 'off')),
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- ─── (c) Drop the 4 summarizer columns from memory_admin_settings ─────
-- The CHECK constraint refers to two of the dropped columns, so it goes
-- first.
ALTER TABLE memory_admin_settings
    DROP CONSTRAINT IF EXISTS summarizer_keep_lt_trigger,
    DROP COLUMN IF EXISTS summarize_after_tokens,
    DROP COLUMN IF EXISTS summarizer_keep_recent_tokens,
    DROP COLUMN IF EXISTS full_summary_prompt,
    DROP COLUMN IF EXISTS incremental_summary_prompt;
