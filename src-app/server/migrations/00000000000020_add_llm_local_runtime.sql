-- Add deployment configuration to llm_providers
ALTER TABLE llm_providers
ADD COLUMN deployment_config JSONB DEFAULT '{
  "type": "local",
  "binary_path": null
}'::jsonb;

-- Runtime instance tracking (one instance per model)
CREATE TABLE llm_runtime_instances (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- Core relationships (model_id is unique - one instance per model)
    model_id UUID NOT NULL UNIQUE REFERENCES llm_models(id) ON DELETE CASCADE,
    provider_id UUID NOT NULL REFERENCES llm_providers(id) ON DELETE CASCADE,

    -- Local access details
    local_port INTEGER NOT NULL,    -- Local port
    base_url VARCHAR(512) NOT NULL, -- http://127.0.0.1:{local_port}/v1

    -- Instance status
    status VARCHAR(50) NOT NULL CHECK (status IN ('starting', 'running', 'stopping', 'stopped', 'failed')),
    error_message TEXT,

    -- Timing information
    started_at TIMESTAMP WITH TIME ZONE DEFAULT NOW() NOT NULL,
    last_health_check TIMESTAMP WITH TIME ZONE,
    stopped_at TIMESTAMP WITH TIME ZONE
);

-- Indexes for runtime instances
CREATE INDEX idx_runtime_instances_provider ON llm_runtime_instances(provider_id);
CREATE INDEX idx_runtime_instances_status ON llm_runtime_instances(status);
