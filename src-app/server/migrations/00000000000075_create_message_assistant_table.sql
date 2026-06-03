-- message_assistant: 1:1 join between messages and assistants,
-- replacing the inline `messages.assistant_id UUID` column so chat's
-- schema no longer mentions assistants. Chat knows nothing about
-- assistants — per-message assistant attribution is recorded here by
-- the assistant module's chat-extension (via the
-- `after_user_message_created` lifecycle hook).
--
-- Purpose: when the user clicks Edit on a past message, the frontend
-- assistant extension restores the originally-selected assistant so
-- the revision goes through the same instructions/template the
-- original message used (edit-fidelity audit trail). Same reason the
-- mcp bridge's `message_mcp_servers` table (migration 74) exists.
--
-- Schema:
--   * PRIMARY KEY on `message_id` (1:1 — a message had at most one
--     assistant; nullable case is represented by row absence rather
--     than a NULL column).
--   * `assistant_id` has NO FK constraint (soft reference). Assistants
--     can be deleted; we keep the audit row so historical attribution
--     survives. Same pattern as `message_mcp_servers.server_id`.
--   * `ON DELETE CASCADE` on `message_id` so deleting a message drops
--     the attribution row automatically.
--
-- Data migration: rows in `messages` with a non-NULL `assistant_id`
-- get one row inserted here before the column is dropped.

CREATE TABLE message_assistant (
    message_id   UUID NOT NULL PRIMARY KEY REFERENCES messages(id) ON DELETE CASCADE,
    assistant_id UUID NOT NULL
);

-- Copy existing per-message assistant attribution into the new table.
INSERT INTO message_assistant (message_id, assistant_id)
SELECT id, assistant_id
FROM messages
WHERE assistant_id IS NOT NULL;

-- Drop the legacy column. No index existed on it.
ALTER TABLE messages DROP COLUMN assistant_id;
