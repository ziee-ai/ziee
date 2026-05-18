-- Add CHECK constraint to usage_mode column to enforce valid values
ALTER TABLE mcp_servers
    ADD CONSTRAINT mcp_servers_usage_mode_check
        CHECK (usage_mode IN ('auto', 'always'));
