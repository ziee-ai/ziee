-- Per-server OAuth 2.1 client_credentials configuration for external HTTP MCP
-- servers (Plan-3 Phase-4). Kept in its own table — secrets live apart from the
-- main mcp_servers row, and adding columns there would ripple through every
-- compile-time sqlx::query! and McpServer literal.
--
-- client_secret is stored as plaintext TEXT, consistent with the existing
-- llm_providers.api_key / user_llm_provider_api_keys.api_key precedent (the
-- codebase has no at-rest encryption layer). Built-in servers never use this
-- table (they authenticate with a short-lived internal JWT instead).
CREATE TABLE mcp_server_oauth_configs (
    server_id     UUID PRIMARY KEY REFERENCES mcp_servers(id) ON DELETE CASCADE,
    client_id     TEXT NOT NULL,
    client_secret TEXT NOT NULL,
    -- Space-separated OAuth scopes; NULL when the server requires none.
    scopes        TEXT,
    -- RFC 8707 resource indicator (usually the MCP server URL); NULL = omit.
    resource      TEXT,
    created_at    TIMESTAMP WITH TIME ZONE DEFAULT NOW() NOT NULL,
    updated_at    TIMESTAMP WITH TIME ZONE DEFAULT NOW() NOT NULL
);

COMMENT ON TABLE mcp_server_oauth_configs IS
    'OAuth 2.1 client_credentials config for external HTTP MCP servers (one row per server).';
