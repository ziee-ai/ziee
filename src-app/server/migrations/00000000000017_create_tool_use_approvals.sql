-- Create tool_use_approvals table
-- Stores pending tool use approvals for manual approval workflow
CREATE TABLE tool_use_approvals (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    conversation_id UUID NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
    branch_id UUID NOT NULL REFERENCES branches(id) ON DELETE CASCADE,
    message_id UUID NOT NULL REFERENCES messages(id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,

    -- Tool use details from LLM response
    tool_use_id VARCHAR(255) NOT NULL, -- ID from LLM ContentBlock::ToolUse
    tool_name VARCHAR(255) NOT NULL,   -- Format: "server_name::tool_name"
    tool_input JSONB NOT NULL,         -- Tool input arguments

    -- Server identification (hybrid approach for referential integrity + readability)
    server_id UUID REFERENCES mcp_servers(id) ON DELETE CASCADE,
    server_name VARCHAR(255) NOT NULL,

    -- Approval status: "pending" | "approved" | "denied" | "cancelled"
    status VARCHAR(50) NOT NULL DEFAULT 'pending',

    -- Approval details
    approved_at TIMESTAMPTZ,
    approved_by UUID REFERENCES users(id) ON DELETE SET NULL,
    approval_note TEXT,

    -- Timestamps
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    -- Unique constraint: one approval record per tool use
    CONSTRAINT unique_tool_use UNIQUE(message_id, tool_use_id)
);

-- Indexes for efficient lookups
CREATE INDEX idx_tool_use_approvals_conversation_id ON tool_use_approvals(conversation_id);
CREATE INDEX idx_tool_use_approvals_branch_id ON tool_use_approvals(branch_id);
CREATE INDEX idx_tool_use_approvals_message_id ON tool_use_approvals(message_id);
CREATE INDEX idx_tool_use_approvals_user_id ON tool_use_approvals(user_id);
CREATE INDEX idx_tool_use_approvals_status ON tool_use_approvals(status);
CREATE INDEX idx_tool_use_approvals_server_id ON tool_use_approvals(server_id);

-- Composite index for finding pending approvals on a branch
CREATE INDEX idx_tool_use_approvals_branch_status ON tool_use_approvals(branch_id, status);

-- Trigger to update updated_at timestamp
CREATE TRIGGER update_tool_use_approvals_updated_at
    BEFORE UPDATE ON tool_use_approvals
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();
