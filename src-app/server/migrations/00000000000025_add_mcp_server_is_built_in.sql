ALTER TABLE mcp_servers ADD COLUMN is_built_in BOOLEAN NOT NULL DEFAULT false;

UPDATE mcp_servers SET is_built_in = true WHERE is_system = true AND name IN ('filesystem', 'fetch', 'browser', 'git');
