-- Create message_contents table for chat module
CREATE TABLE message_contents (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    message_id UUID NOT NULL REFERENCES messages(id) ON DELETE CASCADE,

    -- Content type and data (extensible design)
    content_type VARCHAR(50) NOT NULL,  -- 'text', 'thinking', 'image', etc. (extensions can add more)
    content JSONB NOT NULL,              -- Stores MessageContentData as JSON

    -- Ordering within message
    sequence_order INTEGER NOT NULL DEFAULT 0,

    -- Timestamps
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Indexes for performance
CREATE INDEX idx_message_contents_message_id ON message_contents(message_id);
CREATE INDEX idx_message_contents_sequence ON message_contents(message_id, sequence_order);
CREATE INDEX idx_message_contents_type ON message_contents(content_type);
CREATE INDEX idx_message_contents_content ON message_contents USING gin(content);
