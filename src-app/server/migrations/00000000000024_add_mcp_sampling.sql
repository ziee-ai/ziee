-- Add MCP sampling support columns to mcp_servers table
-- supports_sampling: enables MCP sampling protocol (server requests LLM completions inline)
-- usage_mode: 'auto' (LLM decides when to use) or 'always' (pre-process every prompt)
-- max_concurrent_sessions: limit simultaneous sessions (NULL = unlimited)
ALTER TABLE mcp_servers
    ADD COLUMN supports_sampling BOOLEAN NOT NULL DEFAULT false,
    ADD COLUMN usage_mode VARCHAR(50) NOT NULL DEFAULT 'auto',
    ADD COLUMN max_concurrent_sessions INTEGER NULL;
