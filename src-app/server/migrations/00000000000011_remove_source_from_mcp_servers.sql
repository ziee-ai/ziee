-- Remove source column from mcp_servers table
ALTER TABLE mcp_servers DROP COLUMN IF EXISTS source;

-- Remove the check constraint if it exists
ALTER TABLE mcp_servers DROP CONSTRAINT IF EXISTS check_source_structure;
