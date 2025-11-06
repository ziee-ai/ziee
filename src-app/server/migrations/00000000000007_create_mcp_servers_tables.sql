-- Create mcp_servers table (management-only, no runtime)
CREATE TABLE mcp_servers (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID REFERENCES users(id) ON DELETE CASCADE,  -- NULL for system servers
    name VARCHAR(255) NOT NULL,
    display_name VARCHAR(255) NOT NULL,
    description TEXT,
    enabled BOOLEAN DEFAULT true NOT NULL,
    is_system BOOLEAN DEFAULT false NOT NULL,

    -- Transport configuration
    transport_type VARCHAR(50) NOT NULL DEFAULT 'stdio',

    -- stdio transport
    command TEXT,
    args JSONB DEFAULT '[]',
    environment_variables JSONB DEFAULT '{}',

    -- http/sse transport
    url TEXT,
    headers JSONB DEFAULT '{}',

    -- Runtime configuration
    timeout_seconds INTEGER DEFAULT 30 NOT NULL,

    -- Metadata
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW() NOT NULL,
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW() NOT NULL,

    -- Constraints
    CONSTRAINT unique_mcp_server_name UNIQUE (name, user_id, is_system),
    CONSTRAINT system_server_must_have_no_owner CHECK (
        (is_system = true AND user_id IS NULL) OR
        (is_system = false AND user_id IS NOT NULL)
    ),
    CONSTRAINT valid_transport_config CHECK (
        (transport_type = 'stdio' AND command IS NOT NULL) OR
        (transport_type IN ('http', 'sse') AND url IS NOT NULL)
    )
);

-- Create user_group_mcp_servers table
CREATE TABLE user_group_mcp_servers (
    group_id UUID NOT NULL REFERENCES groups(id) ON DELETE CASCADE,
    mcp_server_id UUID NOT NULL REFERENCES mcp_servers(id) ON DELETE CASCADE,
    assigned_at TIMESTAMP WITH TIME ZONE DEFAULT NOW() NOT NULL,
    PRIMARY KEY (group_id, mcp_server_id)
);

-- Indexes
CREATE INDEX idx_mcp_servers_user_id ON mcp_servers(user_id);
CREATE INDEX idx_mcp_servers_is_system ON mcp_servers(is_system);
CREATE INDEX idx_mcp_servers_enabled ON mcp_servers(enabled);
CREATE INDEX idx_mcp_servers_transport_type ON mcp_servers(transport_type);

CREATE INDEX idx_group_mcp_servers_group_id ON user_group_mcp_servers(group_id);
CREATE INDEX idx_group_mcp_servers_server_id ON user_group_mcp_servers(mcp_server_id);

-- Insert default system servers (disabled by default except fetch which is assigned to default group)
INSERT INTO mcp_servers (name, display_name, description, transport_type, is_system, enabled, command, args, environment_variables) VALUES
('filesystem', 'Filesystem Access', 'Access local filesystem operations', 'stdio', true, false, 'npx', '["-y", "@modelcontextprotocol/server-filesystem"]', '{}'),
('fetch', 'Web Fetch', 'Fetch content from web URLs', 'stdio', true, true, 'uvx', '["mcp-server-fetch"]', '{}'),
('browser', 'Browser Automation', 'Automate browser interactions', 'stdio', true, false, 'npx', '["@browsermcp/mcp"]', '{}'),
('git', 'Git Operations', 'Git repository operations', 'stdio', true, false, 'npx', '["-y", "mcp-git-server"]', '{}');

-- Assign fetch MCP server to default user group
INSERT INTO user_group_mcp_servers (group_id, mcp_server_id)
SELECT g.id, s.id
FROM groups g, mcp_servers s
WHERE g.is_default = true
  AND s.name = 'fetch'
  AND s.is_system = true;
