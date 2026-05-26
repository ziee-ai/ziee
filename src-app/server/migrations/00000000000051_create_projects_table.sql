-- Projects table: a flat, per-user grouping above conversations.
-- Holds persistent context (instructions + files via project_files join +
-- default assistant/model + inline MCP defaults) that the project chat
-- extension injects into every conversation in the project.
--
-- Design notes:
--   * Per-user only in v1 (locked); no sharing, no templates, no nesting.
--     `user_id` CASCADE matches existing conversations + assistants.
--   * MCP defaults stored INLINE (mcp_*) rather than a sibling
--     project_mcp_settings table — projects are created with these from
--     day one, so a 1:1 sibling adds no value and creates a sync hazard.
--   * `default_assistant_id` / `default_model_id` are nullable FKs with
--     ON DELETE SET NULL: deleting the underlying entity blanks the
--     default but preserves the project.
--   * Per-user UNIQUE on `name` so a user's project list can be looked
--     up by name; matches Claude/ChatGPT UX.
--   * Length caps (description 4 KiB, instructions 64 KiB) live at the
--     handler layer, not DB CHECK, to keep validation error messages
--     uniform with the assistant module.

CREATE TABLE projects (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,

    name VARCHAR(255) NOT NULL,
    description TEXT,
    instructions TEXT,

    default_assistant_id UUID REFERENCES assistants(id) ON DELETE SET NULL,
    default_model_id UUID REFERENCES llm_models(id) ON DELETE SET NULL,

    -- MCP defaults inline (shapes mirror conversation_mcp_settings)
    mcp_approval_mode VARCHAR(50) NOT NULL DEFAULT 'manual_approve',
    mcp_auto_approved_tools JSONB NOT NULL DEFAULT '[]'::jsonb,
    mcp_disabled_servers JSONB NOT NULL DEFAULT '[]'::jsonb,

    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    CONSTRAINT projects_user_name_unique UNIQUE (user_id, name)
);

CREATE INDEX idx_projects_user_id ON projects(user_id);
CREATE INDEX idx_projects_updated_at ON projects(updated_at DESC);

CREATE TRIGGER update_projects_updated_at
    BEFORE UPDATE ON projects
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();
