-- Create messages table for chat module
-- Uses junction table (branch_messages) instead of direct branch_id FK
-- to enable copy-on-write branching
CREATE TABLE messages (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- Message metadata
    role VARCHAR(20) NOT NULL,  -- 'user', 'assistant', 'system'

    -- Message hierarchy (for threading)
    parent_id UUID REFERENCES messages(id) ON DELETE CASCADE,
    sequence_number INTEGER NOT NULL DEFAULT 0,

    -- Edit tracking (for message edit lineage)
    originated_from_id UUID NOT NULL,
    edit_count INTEGER NOT NULL DEFAULT 0,

    -- Timestamps
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Indexes for performance
CREATE INDEX idx_messages_parent_id ON messages(parent_id);
CREATE INDEX idx_messages_created_at ON messages(created_at DESC);
CREATE INDEX idx_messages_role ON messages(role);
CREATE INDEX idx_messages_originated_from_id ON messages(originated_from_id);

-- Comments
COMMENT ON COLUMN messages.originated_from_id IS 'UUID of the original message in an edit lineage. All edits of the same message share this ID.';
COMMENT ON COLUMN messages.edit_count IS 'Number of times any message in this edit lineage has been edited. Increments for all messages with the same originated_from_id.';
