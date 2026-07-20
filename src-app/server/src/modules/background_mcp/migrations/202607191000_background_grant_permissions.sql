-- background_mcp (ITEM-17 / DEC-33): grant its domain permission to the owning
-- group. `background::use` gates the built-in background-run MCP surface
-- (spawn_background / check_status / collect_result). Administrators already
-- hold it via the `*` wildcard; every ordinary user gets it here so the model
-- can reach the trio in any tool-capable chat. Idempotent (DISTINCT unnest).

UPDATE groups
SET permissions = ARRAY(SELECT DISTINCT unnest(permissions || ARRAY['background::use'])),
    updated_at = NOW()
WHERE name = 'Users' AND is_system = TRUE;
