-- message_mcp_servers: M:N join between messages and MCP servers,
-- replacing the inline `messages.mcp_server_ids UUID[]` column so
-- chat's schema no longer mentions MCP. Chat knows nothing about
-- MCP — per-message server selection is recorded here by the mcp
-- module's chat-extension (via the new `after_user_message_created`
-- lifecycle hook).
--
-- Purpose: when the user clicks Edit on a past message, the frontend
-- mcp extension restores the originally-enabled MCP server set so the
-- revision goes to the same toolset that produced the original
-- response (edit-fidelity audit trail).
--
-- Schema:
--   * PRIMARY KEY (message_id, server_id) — a server can only appear
--     once per message; order doesn't matter (set semantics, matching
--     the frontend's `setEnabledServers(serverIds: string[])` API).
--   * `server_id` has NO FK constraint (soft reference). MCP servers
--     can be deleted or rotated; we keep the audit row so the
--     historical "this message used server X" information survives.
--     This mirrors how `messages.model_id` and `messages.assistant_id`
--     are treated as soft references.
--   * `ON DELETE CASCADE` on `message_id` so deleting a message drops
--     its server-list rows automatically.
--
-- Data migration: any rows in `messages` that had a non-empty
-- `mcp_server_ids` array get one row inserted here per server before
-- the column is dropped.

CREATE TABLE message_mcp_servers (
    message_id UUID NOT NULL REFERENCES messages(id) ON DELETE CASCADE,
    server_id  UUID NOT NULL,
    PRIMARY KEY (message_id, server_id)
);

-- Copy existing per-message server lists into the new join table.
INSERT INTO message_mcp_servers (message_id, server_id)
SELECT id, UNNEST(mcp_server_ids)
FROM messages
WHERE mcp_server_ids IS NOT NULL
  AND array_length(mcp_server_ids, 1) > 0;

-- Drop the legacy column. No index existed on it.
ALTER TABLE messages DROP COLUMN mcp_server_ids;
