-- conversation_memory_settings: per-conversation memory-feature toggle,
-- replacing the inline `conversations.memory_mode TEXT` column so chat's
-- schema no longer mentions memory vocabulary. Chat knows nothing about
-- memory — per-conversation memory_mode is owned and validated here by
-- the memory module's chat-extension bridge.
--
-- Purpose: each conversation can override the user-level memory toggle
-- (extract/retrieve on or off). Three values:
--   * 'inherit' — follow the user's default (this is the implicit
--     default; rows with this value are NOT stored, row absence == 'inherit').
--   * 'on'      — force memory on for this conversation.
--   * 'off'     — force memory off for this conversation.
--
-- Schema:
--   * PRIMARY KEY on `conversation_id` (1:1 — at most one row per
--     conversation; absence == 'inherit').
--   * `ON DELETE CASCADE` on `conversation_id` so deleting a
--     conversation drops the override automatically.
--   * `CHECK` constraint moves chat's prior handler-layer validation
--     ('inherit'/'on'/'off') into the schema — neither chat nor
--     memory has to re-validate at the API layer.
--
-- Data migration: rows in `conversations` with `memory_mode <> 'inherit'`
-- get one row inserted here before the column is dropped. Default-
-- valued rows are not migrated (saves storage; row absence carries the
-- same semantics).

CREATE TABLE conversation_memory_settings (
    conversation_id UUID NOT NULL PRIMARY KEY REFERENCES conversations(id) ON DELETE CASCADE,
    memory_mode     TEXT NOT NULL CHECK (memory_mode IN ('inherit', 'on', 'off'))
);

-- Backfill non-default rows only (row absence == 'inherit').
INSERT INTO conversation_memory_settings (conversation_id, memory_mode)
SELECT id, memory_mode
FROM conversations
WHERE memory_mode <> 'inherit';

-- Drop the legacy column. No index existed on it.
ALTER TABLE conversations DROP COLUMN memory_mode;
