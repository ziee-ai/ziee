-- Create branches table for chat module
CREATE TABLE branches (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    conversation_id UUID NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,

    -- Branch hierarchy
    parent_branch_id UUID REFERENCES branches(id) ON DELETE SET NULL,
    created_from_message_id UUID,  -- Which message triggered this branch (for edit/regenerate)

    -- Timestamps
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Indexes for performance
CREATE INDEX idx_branches_conversation_id ON branches(conversation_id);
CREATE INDEX idx_branches_parent_branch_id ON branches(parent_branch_id);
CREATE INDEX idx_branches_created_from_message_id ON branches(created_from_message_id);

-- Add FK to conversations table (completing circular reference)
ALTER TABLE conversations
    ADD CONSTRAINT fk_conversations_active_branch
    FOREIGN KEY (active_branch_id) REFERENCES branches(id) ON DELETE SET NULL;
