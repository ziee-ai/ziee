-- Per-conversation memory mode override.
--
-- Default 'inherit' = follow the user's `user_memory_settings.retrieval_enabled`.
-- 'on'  = force retrieval for this conversation regardless.
-- 'off' = suppress retrieval for this conversation regardless.
--
-- Extraction still follows user settings — the per-conversation toggle
-- only controls retrieval/injection at chat time.
ALTER TABLE conversations
    ADD COLUMN memory_mode TEXT NOT NULL DEFAULT 'inherit'
        CHECK (memory_mode IN ('inherit', 'on', 'off'));
