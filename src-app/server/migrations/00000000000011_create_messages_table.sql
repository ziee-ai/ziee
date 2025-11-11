-- Create messages table for chat module
CREATE TABLE messages (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    branch_id UUID NOT NULL REFERENCES branches(id) ON DELETE CASCADE,

    -- Message metadata
    role VARCHAR(20) NOT NULL,  -- 'user', 'assistant', 'system'

    -- Message hierarchy (for threading)
    parent_id UUID REFERENCES messages(id) ON DELETE CASCADE,
    sequence_number INTEGER NOT NULL DEFAULT 0,

    -- Timestamps
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Indexes for performance
CREATE INDEX idx_messages_branch_id ON messages(branch_id);
CREATE INDEX idx_messages_parent_id ON messages(parent_id);
CREATE INDEX idx_messages_sequence ON messages(branch_id, sequence_number);
CREATE INDEX idx_messages_created_at ON messages(created_at DESC);
CREATE INDEX idx_messages_role ON messages(role);
