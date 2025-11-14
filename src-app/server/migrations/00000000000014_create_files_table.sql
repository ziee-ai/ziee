-- Create files table (completely independent from conversations/projects)
CREATE TABLE files (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    filename VARCHAR(255) NOT NULL,
    file_size BIGINT NOT NULL,
    mime_type VARCHAR(100),
    checksum VARCHAR(64),
    thumbnail_count INTEGER DEFAULT 0 NOT NULL,
    page_count INTEGER DEFAULT 0 NOT NULL,
    processing_metadata JSONB DEFAULT '{}',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Indexes for efficient queries
CREATE INDEX idx_files_user_id ON files(user_id);
CREATE INDEX idx_files_mime_type ON files(mime_type);
CREATE INDEX idx_files_created_at ON files(created_at DESC);
CREATE INDEX idx_files_file_size ON files(file_size);
CREATE INDEX idx_files_checksum ON files(checksum);
CREATE INDEX idx_files_processing_metadata ON files USING GIN(processing_metadata);
