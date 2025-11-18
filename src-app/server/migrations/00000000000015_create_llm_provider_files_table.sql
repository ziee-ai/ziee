-- Provider file mapping table for caching uploaded files
-- Enables file reuse across messages and cost optimization
CREATE TABLE llm_provider_files (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- Foreign keys
    file_id UUID NOT NULL REFERENCES files(id) ON DELETE CASCADE,
    provider_id UUID NOT NULL REFERENCES llm_providers(id) ON DELETE CASCADE,

    -- Provider's file identifier (from their Files API)
    -- NULL if upload pending or failed
    -- Examples:
    --   Anthropic: "file_abc123"
    --   Gemini: "https://generativelanguage.googleapis.com/v1beta/files/xyz789"
    --   OpenAI: NULL (not used for vision chat)
    provider_file_id VARCHAR(512),

    -- Provider-specific metadata stored as JSONB
    -- Example structure:
    -- {
    --   "uploaded_at": "2025-01-14T12:00:00Z",
    --   "expires_at": "2025-01-16T12:00:00Z",  -- Gemini only (48h)
    --   "filename": "document.pdf",
    --   "mime_type": "application/pdf",
    --   "file_size": 1024000,
    --   "api_key_hash": "sha256...",  -- Optional: track which key
    --   "workspace_id": "workspace_123",  -- Anthropic workspace
    --   "upload_error": "quota exceeded"  -- If failed
    -- }
    provider_metadata JSONB DEFAULT '{}' NOT NULL,

    -- Upload status for tracking
    upload_status VARCHAR(50) DEFAULT 'pending' NOT NULL
        CHECK (upload_status IN (
            'pending',      -- Queued for upload
            'uploading',    -- Upload in progress
            'completed',    -- Successfully uploaded
            'failed',       -- Upload failed
            'expired'       -- File expired (Gemini 48h)
        )),

    -- Timestamps
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    -- Unique constraint: one mapping per file+provider combination
    UNIQUE(file_id, provider_id)
);

-- Indexes for efficient queries
CREATE INDEX idx_llm_provider_files_file_id
    ON llm_provider_files(file_id);

CREATE INDEX idx_llm_provider_files_provider_id
    ON llm_provider_files(provider_id);

CREATE INDEX idx_llm_provider_files_status
    ON llm_provider_files(upload_status);

-- GIN index for JSONB metadata queries
CREATE INDEX idx_llm_provider_files_metadata
    ON llm_provider_files USING GIN(provider_metadata);

-- Specialized index for Gemini expiration tracking
CREATE INDEX idx_llm_provider_files_expires_at
    ON llm_provider_files((provider_metadata->>'expires_at'))
    WHERE provider_metadata->>'expires_at' IS NOT NULL;

-- Trigger for auto-updating updated_at
CREATE TRIGGER update_llm_provider_files_updated_at
    BEFORE UPDATE ON llm_provider_files
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();

-- Comments for documentation
COMMENT ON TABLE llm_provider_files IS
    'Maps system files to provider-specific file IDs for caching and reuse';
COMMENT ON COLUMN llm_provider_files.provider_file_id IS
    'File ID/URI returned by providers Files API (Anthropic, Gemini)';
COMMENT ON COLUMN llm_provider_files.provider_metadata IS
    'Provider-specific metadata: upload time, expiration, workspace, errors';
COMMENT ON COLUMN llm_provider_files.upload_status IS
    'Current upload status: pending, uploading, completed, failed, expired';
