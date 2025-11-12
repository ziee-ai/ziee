-- Create branch_messages junction table for copy-on-write branching
-- This enables messages to belong to multiple branches (via cloning)
CREATE TABLE branch_messages (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    branch_id UUID NOT NULL REFERENCES branches(id) ON DELETE CASCADE,
    message_id UUID NOT NULL REFERENCES messages(id) ON DELETE CASCADE,
    is_clone BOOLEAN NOT NULL DEFAULT false,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,

    -- Ensure a message can only be in a branch once
    UNIQUE(branch_id, message_id)
);

-- Create indexes for efficient branch queries
CREATE INDEX idx_branch_messages_branch_id ON branch_messages(branch_id, created_at);
CREATE INDEX idx_branch_messages_message_id ON branch_messages(message_id);

-- Comment explaining the architecture
COMMENT ON TABLE branch_messages IS 'Junction table for messages and branches. Enables copy-on-write branching where messages can belong to multiple branches (via cloning). is_clone=true means message is referenced from another branch, is_clone=false means it was created in this branch.';
