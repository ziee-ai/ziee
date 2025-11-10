-- Migrate existing source data from core tables to hub_entities
-- This must run BEFORE dropping source columns

-- Migrate assistants with hub source
INSERT INTO hub_entities (entity_type, entity_id, hub_id, hub_category, created_by)
SELECT
    'assistant' as entity_type,
    id as entity_id,
    source->>'id' as hub_id,
    'assistant' as hub_category,
    created_by
FROM assistants
WHERE source IS NOT NULL
  AND source->>'type' = 'hub'
  AND source->>'id' IS NOT NULL
ON CONFLICT (entity_type, entity_id) DO NOTHING;

-- Migrate mcp_servers with hub source
INSERT INTO hub_entities (entity_type, entity_id, hub_id, hub_category, created_by)
SELECT
    'mcp_server' as entity_type,
    id as entity_id,
    source->>'id' as hub_id,
    'mcp_server' as hub_category,
    user_id as created_by
FROM mcp_servers
WHERE source IS NOT NULL
  AND source->>'type' = 'hub'
  AND source->>'id' IS NOT NULL
ON CONFLICT (entity_type, entity_id) DO NOTHING;

-- Migrate llm_models with hub source
INSERT INTO hub_entities (entity_type, entity_id, hub_id, hub_category, created_by)
SELECT
    'llm_model' as entity_type,
    id as entity_id,
    source->>'id' as hub_id,
    'model' as hub_category,
    NULL as created_by  -- Models are system-wide
FROM llm_models
WHERE source IS NOT NULL
  AND source->>'type' = 'hub'
  AND source->>'id' IS NOT NULL
ON CONFLICT (entity_type, entity_id) DO NOTHING;

-- Verify migration
DO $$
DECLARE
    assistants_count INT;
    mcp_count INT;
    models_count INT;
    hub_count INT;
BEGIN
    SELECT COUNT(*) INTO assistants_count FROM assistants WHERE source->>'type' = 'hub';
    SELECT COUNT(*) INTO mcp_count FROM mcp_servers WHERE source->>'type' = 'hub';
    SELECT COUNT(*) INTO models_count FROM llm_models WHERE source->>'type' = 'hub';
    SELECT COUNT(*) INTO hub_count FROM hub_entities;

    RAISE NOTICE 'Migration Summary:';
    RAISE NOTICE '  Assistants with hub source: %', assistants_count;
    RAISE NOTICE '  MCP servers with hub source: %', mcp_count;
    RAISE NOTICE '  LLM models with hub source: %', models_count;
    RAISE NOTICE '  Total hub_entities records: %', hub_count;
END $$;
