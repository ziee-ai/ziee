-- Create conversations table for chat module
CREATE TABLE conversations (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,

    -- Model configuration (optional, for display/history purposes)
    -- Actual model selection happens per-message via SendMessageRequest
    model_id UUID REFERENCES llm_models(id) ON DELETE SET NULL,

    -- Conversation metadata
    title VARCHAR(500),

    -- Active branch tracking (FK added after branches table created)
    active_branch_id UUID,

    -- Timestamps
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Indexes for performance
CREATE INDEX idx_conversations_user_id ON conversations(user_id);
CREATE INDEX idx_conversations_created_at ON conversations(created_at DESC);
CREATE INDEX idx_conversations_model_id ON conversations(model_id);
