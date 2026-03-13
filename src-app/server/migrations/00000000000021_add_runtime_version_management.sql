-- Runtime binary version management
-- Tracks downloaded runtime binaries (llama-server, mistralrs-server)

-- Table to track runtime binary versions
CREATE TABLE llm_runtime_versions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- Version identification
    engine VARCHAR(50) NOT NULL CHECK (engine IN ('llamacpp', 'mistralrs')),
    version VARCHAR(100) NOT NULL, -- e.g., 'v0.7.0', 'v0.9.0', 'latest'

    -- Platform details
    platform VARCHAR(50) NOT NULL,      -- 'linux', 'darwin', 'windows'
    arch VARCHAR(50) NOT NULL,          -- 'x86_64', 'aarch64'
    backend VARCHAR(50) NOT NULL,       -- 'cpu', 'cuda', 'rocm', 'metal'

    -- Binary information
    binary_path TEXT NOT NULL,          -- Path in ~/.llm-runtime/binaries/

    -- System defaults (one per engine)
    is_system_default BOOLEAN NOT NULL DEFAULT FALSE,

    -- Timestamps
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW() NOT NULL,

    -- Unique constraint: one version per engine/version/platform/arch/backend combo
    UNIQUE(engine, version, platform, arch, backend)
);

-- Indexes for efficient querying
CREATE INDEX idx_runtime_versions_engine ON llm_runtime_versions(engine);
CREATE INDEX idx_runtime_versions_default ON llm_runtime_versions(is_system_default) WHERE is_system_default = TRUE;

-- Add version tracking to llm_providers (provider-level defaults)
ALTER TABLE llm_providers
ADD COLUMN default_runtime_version_id UUID REFERENCES llm_runtime_versions(id) ON DELETE SET NULL;

-- Add version tracking to llm_models (model-specific pinning)
ALTER TABLE llm_models
ADD COLUMN required_runtime_version_id UUID REFERENCES llm_runtime_versions(id) ON DELETE SET NULL;

-- Update llm_runtime_instances to track which version was used
ALTER TABLE llm_runtime_instances
ADD COLUMN runtime_version_id UUID REFERENCES llm_runtime_versions(id) ON DELETE SET NULL,
-- Remove deployment_type column since it's now determined by the runtime version
DROP COLUMN IF EXISTS deployment_type;

-- Index for provider default versions
CREATE INDEX idx_providers_default_runtime_version ON llm_providers(default_runtime_version_id) WHERE default_runtime_version_id IS NOT NULL;

-- Index for model required versions
CREATE INDEX idx_models_required_runtime_version ON llm_models(required_runtime_version_id) WHERE required_runtime_version_id IS NOT NULL;

-- Index for instance runtime versions
CREATE INDEX idx_runtime_instances_version ON llm_runtime_instances(runtime_version_id);
