-- ===============================
-- LLM MODELS MIGRATION
-- ===============================
-- This migration creates tables for LLM model management:
-- NOTE: llm_repositories table already exists from migration 00000000000002
-- 1. llm_models: Unified model table for all model types
-- 2. llm_model_files: Individual files within models
-- 3. llm_download_instances: Track model download progress

-- ===============================
-- 1. LLM MODELS
-- ===============================

-- Create model provider models table (unified table for all model types)
CREATE TABLE llm_models (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    provider_id UUID NOT NULL REFERENCES llm_providers(id) ON DELETE CASCADE,
    name VARCHAR(255) NOT NULL,
    display_name VARCHAR(255) NOT NULL,
    description TEXT,
    enabled BOOLEAN DEFAULT TRUE NOT NULL,
    is_deprecated BOOLEAN DEFAULT FALSE NOT NULL,
    is_active BOOLEAN DEFAULT FALSE NOT NULL, -- For local start/stop state
    capabilities JSONB DEFAULT '{}',
    parameters JSONB DEFAULT '{}',
    -- Model file information
    file_size_bytes BIGINT,
    validation_status VARCHAR(50) CHECK (validation_status IN (
        'pending',        -- Initial status when model is created
        'await_upload',   -- For local folder uploads waiting for files
        'downloading',    -- For Hugging Face downloads in progress
        'processing',     -- While processing uploaded files
        'completed',      -- Successfully downloaded/processed
        'failed',         -- Download/processing failed
        'valid',          -- After successful validation
        'invalid',        -- After failed validation
        'error',          -- General error state
        'validation_warning' -- Downloaded but with validation warnings
    )),
    validation_issues JSONB,
    -- Engine-specific settings
    engine_type VARCHAR(50) NOT NULL DEFAULT 'mistralrs',
    engine_settings JSONB DEFAULT NULL, -- Consolidated engine settings
    file_format VARCHAR(20) NOT NULL DEFAULT 'safetensors',
    source JSONB DEFAULT NULL,
    -- Port number where the model server is running (for local models)
    port INTEGER,
    -- Process ID of the running model server (for local models)
    pid INTEGER,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP NOT NULL,
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP NOT NULL,
    CONSTRAINT llm_models_display_name_not_empty CHECK (display_name != ''),
    CONSTRAINT llm_models_provider_id_name_unique UNIQUE (provider_id, name),
    CONSTRAINT check_engine_type CHECK (engine_type IN ('mistralrs', 'llamacpp', 'none')),
    CONSTRAINT check_file_format CHECK (file_format IN ('safetensors', 'pytorch', 'gguf')),
    CONSTRAINT check_source_structure CHECK (
        source IS NULL OR (
            source ? 'type' AND
            (source->>'type' = 'manual' OR source->>'type' = 'hub')
        )
    )
);

-- ===============================
-- 2. MODEL FILES SYSTEM
-- ===============================

-- Create model_files table for tracking individual files within models
CREATE TABLE llm_model_files (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    model_id UUID NOT NULL REFERENCES llm_models(id) ON DELETE CASCADE,
    filename VARCHAR(500) NOT NULL,
    file_path VARCHAR(1000) NOT NULL,
    file_size_bytes BIGINT NOT NULL,
    file_type VARCHAR(50) NOT NULL,
    upload_status VARCHAR(50) DEFAULT 'pending' CHECK (upload_status IN ('pending', 'uploading', 'completed', 'failed')) NOT NULL,
    uploaded_at TIMESTAMP WITH TIME ZONE DEFAULT NOW() NOT NULL,
    UNIQUE(model_id, filename)
);

-- ===============================
-- 3. DOWNLOAD INSTANCES
-- ===============================

-- Create download_instances table for tracking model downloads
CREATE TABLE llm_download_instances (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    provider_id UUID NOT NULL REFERENCES llm_providers(id) ON DELETE CASCADE,
    repository_id UUID NOT NULL REFERENCES llm_repositories(id) ON DELETE CASCADE,
    request_data JSONB NOT NULL, -- Stores all download parameters
    status VARCHAR(50) NOT NULL CHECK (status IN ('pending', 'downloading', 'completed', 'failed', 'cancelled')),
    progress_data JSONB DEFAULT '{}', -- Stores phase, current, total, message
    error_message TEXT,
    started_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP NOT NULL,
    completed_at TIMESTAMP WITH TIME ZONE,
    model_id UUID REFERENCES llm_models(id) ON DELETE SET NULL, -- Nullable, filled when download completes
    created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP NOT NULL,
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP NOT NULL
);

-- ===============================
-- 4. INDEXES FOR PERFORMANCE
-- ===============================

-- Indexes for llm_models table
CREATE INDEX idx_llm_models_provider_id ON llm_models(provider_id);
CREATE INDEX idx_llm_models_enabled ON llm_models(enabled);
CREATE INDEX idx_llm_models_validation_status ON llm_models(validation_status);
CREATE INDEX idx_llm_models_engine_type ON llm_models(engine_type);
CREATE INDEX idx_llm_models_created_at ON llm_models(created_at DESC);

-- Indexes for llm_model_files table
CREATE INDEX idx_llm_model_files_model_id ON llm_model_files(model_id);
CREATE INDEX idx_llm_model_files_upload_status ON llm_model_files(upload_status);

-- Indexes for llm_download_instances table
CREATE INDEX idx_llm_download_instances_provider_id ON llm_download_instances(provider_id);
CREATE INDEX idx_llm_download_instances_repository_id ON llm_download_instances(repository_id);
CREATE INDEX idx_llm_download_instances_model_id ON llm_download_instances(model_id);
CREATE INDEX idx_llm_download_instances_status ON llm_download_instances(status);
CREATE INDEX idx_llm_download_instances_started_at ON llm_download_instances(started_at DESC);
