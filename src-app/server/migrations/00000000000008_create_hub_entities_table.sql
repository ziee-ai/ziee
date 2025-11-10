-- Hub entity tracking table
-- Tracks which entities (assistants, mcp_servers, llm_models) were created from hub
CREATE TABLE hub_entities (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- Entity identification
    entity_type VARCHAR(50) NOT NULL,  -- 'assistant', 'mcp_server', 'llm_model'
    entity_id UUID NOT NULL,

    -- Hub source
    hub_id VARCHAR(255) NOT NULL,  -- ID from hub catalog (e.g., 'code-assistant-v1')
    hub_category VARCHAR(50) NOT NULL,  -- 'assistant', 'mcp_server', 'model'

    -- Metadata
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW() NOT NULL,
    created_by UUID REFERENCES users(id) ON DELETE SET NULL,  -- User who created from hub

    -- Constraints
    CONSTRAINT unique_entity_hub_tracking UNIQUE(entity_type, entity_id),
    CONSTRAINT valid_entity_type CHECK (entity_type IN ('assistant', 'mcp_server', 'llm_model')),
    CONSTRAINT valid_hub_category CHECK (hub_category IN ('assistant', 'mcp_server', 'model'))
);

-- Index for fast lookups by entity
CREATE INDEX idx_hub_entities_lookup ON hub_entities(entity_type, entity_id);

-- Index for reverse lookup (find all entities from a hub item)
CREATE INDEX idx_hub_entities_hub_id ON hub_entities(hub_id, entity_type);

-- Index for user tracking
CREATE INDEX idx_hub_entities_user ON hub_entities(created_by) WHERE created_by IS NOT NULL;

-- Comments
COMMENT ON TABLE hub_entities IS 'Tracks which entities were created from hub catalog';
COMMENT ON COLUMN hub_entities.entity_type IS 'Type of entity: assistant, mcp_server, or llm_model';
COMMENT ON COLUMN hub_entities.entity_id IS 'UUID of the created entity';
COMMENT ON COLUMN hub_entities.hub_id IS 'ID from hub catalog (e.g., code-assistant-v1)';
COMMENT ON COLUMN hub_entities.hub_category IS 'Category in hub: assistant, mcp_server, or model';
COMMENT ON COLUMN hub_entities.created_by IS 'User who created this entity from hub (NULL for system-wide like models)';
