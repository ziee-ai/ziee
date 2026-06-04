-- Unify MCP settings into a single scoped table.
--
-- Before this migration:
--   - `conversation_mcp_settings` holds per-conversation MCP config (one row per conv).
--   - `projects.mcp_*` (4 inline columns) holds per-project MCP defaults.
--
-- After:
--   - `mcp_settings` holds BOTH, discriminated by which scope FK is set.
--     `conversation_id` XOR `project_id` is NOT NULL (enforced by CHECK).
--     One row per scope (enforced by per-FK UNIQUE constraints).
--   - `conversation_mcp_settings` table is dropped.
--   - `projects.mcp_*` columns are dropped.
--
-- The shared Rust type `McpSettings` + `McpScope::{Conversation(Uuid), Project(Uuid)}`
-- covers both scopes. The snapshot operation (project's MCP defaults →
-- a new conversation's MCP settings on project-attach) becomes
-- `INSERT INTO mcp_settings (conversation_id, ...) SELECT $conv_id, ...
-- FROM mcp_settings WHERE project_id = $project_id` — single table, single
-- SQL statement.
--
-- Backfill rules:
--   1. Every row in `conversation_mcp_settings` becomes a `mcp_settings`
--      row with `conversation_id` set (preserves IDs + timestamps).
--   2. Only "customized" project rows backfill — projects with bare
--      defaults (manual_approve + empty arrays + null loop_settings)
--      don't need a row; the application returns column defaults when
--      no row exists. Reduces table churn for projects that never opened
--      the MCP modal.
--
-- Owned by mcp module (modules/mcp/settings/). Access via Repos.mcp_settings
-- from both mcp/chat_extension/ (conversation scope) and mcp/project_extension/
-- (project scope).

CREATE TABLE mcp_settings (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    -- Exactly one of these two is NOT NULL (enforced by CHECK below).
    -- ON DELETE CASCADE so deleting a conversation/project drops its
    -- settings row for free — no app-side cleanup needed.
    conversation_id UUID REFERENCES conversations(id) ON DELETE CASCADE,
    project_id      UUID REFERENCES projects(id)      ON DELETE CASCADE,
    user_id         UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,

    -- Payload (same shapes as the two old tables):
    --   approval_mode: 'disabled' | 'auto_approve' | 'manual_approve'
    --   auto_approved_tools: [{"server_id": "uuid", "tools": ["t1", ...]}, ...]
    --   disabled_servers:    [{"server_id": "uuid", "tools": [...]}, ...]
    --     (empty tools = entire server disabled)
    --   loop_settings: object with tool-loop tuning or NULL for "use default"
    approval_mode        VARCHAR(50) NOT NULL DEFAULT 'manual_approve',
    auto_approved_tools  JSONB NOT NULL DEFAULT '[]'::jsonb,
    disabled_servers     JSONB NOT NULL DEFAULT '[]'::jsonb,
    loop_settings        JSONB,

    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    -- Schema invariant: exactly one scope FK is set. (a IS NULL) <> (b IS NULL)
    -- is true iff exactly one is null (and exactly one is not-null).
    CONSTRAINT mcp_settings_one_scope CHECK (
        (conversation_id IS NULL) <> (project_id IS NULL)
    ),
    -- One row per scope. Per-FK constraints rather than a composite
    -- because (NULL, project_id) and (NULL, project_id) are NOT
    -- considered duplicates by a composite UNIQUE.
    CONSTRAINT mcp_settings_one_per_conversation UNIQUE (conversation_id),
    CONSTRAINT mcp_settings_one_per_project      UNIQUE (project_id)
);

CREATE INDEX idx_mcp_settings_user_id ON mcp_settings(user_id);

-- Reuse the shared updated_at trigger (declared in migration 1).
CREATE TRIGGER update_mcp_settings_updated_at
    BEFORE UPDATE ON mcp_settings
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();

-- ─── Backfill from conversation_mcp_settings ──────────────────────────
-- Preserve all rows + ids + timestamps. The old table's UNIQUE
-- (conversation_id) constraint guarantees no dupes; the new table's
-- UNIQUE constraint will hold trivially.
INSERT INTO mcp_settings (
    id, conversation_id, user_id, approval_mode, auto_approved_tools,
    disabled_servers, loop_settings, created_at, updated_at
)
SELECT id, conversation_id, user_id, approval_mode, auto_approved_tools,
       disabled_servers, loop_settings, created_at, updated_at
FROM conversation_mcp_settings;

-- ─── Backfill from projects.mcp_* (customized rows only) ──────────────
-- Skip rows with bare defaults — no need to materialize a row for
-- "everything default". Application reads default-row in memory when no
-- row exists.
INSERT INTO mcp_settings (
    project_id, user_id, approval_mode, auto_approved_tools,
    disabled_servers, loop_settings
)
SELECT id, user_id, mcp_approval_mode, mcp_auto_approved_tools,
       mcp_disabled_servers, mcp_loop_settings
FROM projects
WHERE mcp_approval_mode <> 'manual_approve'
   OR jsonb_array_length(mcp_auto_approved_tools) > 0
   OR jsonb_array_length(mcp_disabled_servers) > 0
   OR mcp_loop_settings IS NOT NULL;

-- ─── Drop the old surfaces ────────────────────────────────────────────
DROP TABLE conversation_mcp_settings;

ALTER TABLE projects
    DROP COLUMN mcp_approval_mode,
    DROP COLUMN mcp_auto_approved_tools,
    DROP COLUMN mcp_disabled_servers,
    DROP COLUMN mcp_loop_settings;
