-- User MCP Defaults
-- Stores user-level default MCP settings that apply to all new conversations

CREATE TABLE user_mcp_defaults (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,

    -- Approval mode: "disabled" | "auto_approve" | "manual_approve"
    approval_mode VARCHAR(50) NOT NULL DEFAULT 'manual_approve',

    -- Auto-approved tools (JSONB array):
    -- Format: [{"server_id": "uuid", "tools": ["tool1", "tool2"]}, ...]
    auto_approved_tools JSONB NOT NULL DEFAULT '[]'::jsonb,

    -- Disabled servers/tools (JSONB array):
    -- Format: [{"server_id": "uuid", "tools": []}, ...]
    -- Empty tools array = entire server disabled
    -- Empty array = all accessible servers enabled (default)
    disabled_servers JSONB NOT NULL DEFAULT '[]'::jsonb,

    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    -- Each user can only have one defaults record
    CONSTRAINT unique_user_mcp_defaults UNIQUE(user_id)
);

-- Index for faster lookups by user
CREATE INDEX idx_user_mcp_defaults_user_id ON user_mcp_defaults(user_id);

-- Trigger to update updated_at
CREATE TRIGGER update_user_mcp_defaults_updated_at
    BEFORE UPDATE ON user_mcp_defaults
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();
