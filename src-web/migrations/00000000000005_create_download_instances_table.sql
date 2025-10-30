-- Create download_instances table for tracking model downloads from repositories
-- Source: react-test/src-tauri/migrations/00000000000001_init.sql
-- Adapted for ziee-chat structure

CREATE TABLE download_instances (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    provider_id UUID NOT NULL REFERENCES llm_providers(id) ON DELETE CASCADE,
    repository_id UUID NOT NULL REFERENCES llm_repositories(id) ON DELETE CASCADE,
    request_data JSONB NOT NULL, -- Stores all download parameters (DownloadRequestData)
    status VARCHAR(50) NOT NULL CHECK (status IN ('pending', 'downloading', 'completed', 'failed', 'cancelled')),
    progress_data JSONB DEFAULT '{}', -- Stores phase, current, total, message, speed_bps, eta_seconds
    error_message TEXT,
    started_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP NOT NULL,
    completed_at TIMESTAMP WITH TIME ZONE,
    model_id UUID REFERENCES llm_models(id) ON DELETE SET NULL, -- Nullable, filled when download completes successfully
    created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP NOT NULL,
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP NOT NULL
);

-- Create index on status for efficient filtering
CREATE INDEX idx_download_instances_status ON download_instances(status);

-- Create index on provider_id for filtering by provider
CREATE INDEX idx_download_instances_provider_id ON download_instances(provider_id);

-- Create index on repository_id for filtering by repository
CREATE INDEX idx_download_instances_repository_id ON download_instances(repository_id);

-- Create index on created_at for sorting
CREATE INDEX idx_download_instances_created_at ON download_instances(created_at DESC);
