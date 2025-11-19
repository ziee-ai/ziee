-- Create conversation_mcp_settings table
-- Stores per-conversation MCP configuration including approval settings
CREATE TABLE conversation_mcp_settings (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    conversation_id UUID NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,

    -- Approval mode: "disabled" | "auto_approve" | "manual_approve"
    approval_mode VARCHAR(50) NOT NULL DEFAULT 'manual_approve',

    -- Auto-approved tools (JSONB array supporting 3 formats):
    --   1. String (legacy): "server_name::tool_name"
    --   2. Object with ID: {"server_id": "uuid", "tool_name": "name"}
    --   3. Object with name: {"server_name": "name", "tool_name": "name"}
    -- Example: ["server::tool1", {"server_id": "uuid", "tool_name": "tool2"}]
    -- Empty array means no tools are auto-approved
    auto_approved_tools JSONB NOT NULL DEFAULT '[]'::jsonb,

    -- Timestamps
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    -- Unique constraint: one settings record per conversation
    CONSTRAINT unique_conversation_settings UNIQUE(conversation_id)
);

-- Indexes for efficient lookups
CREATE INDEX idx_conversation_mcp_settings_conversation_id ON conversation_mcp_settings(conversation_id);
CREATE INDEX idx_conversation_mcp_settings_user_id ON conversation_mcp_settings(user_id);

-- Trigger to update updated_at timestamp
CREATE TRIGGER update_conversation_mcp_settings_updated_at
    BEFORE UPDATE ON conversation_mcp_settings
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();
