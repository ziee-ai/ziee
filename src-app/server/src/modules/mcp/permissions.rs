use crate::modules::permissions::PermissionCheck;

// =====================================================
// User MCP Server Permissions
// =====================================================

/// Permission to view MCP servers (own servers + accessible system servers)
pub struct McpServersRead;
impl PermissionCheck for McpServersRead {
    const NAME: &'static str = "McpServersRead";
    const PERMISSION: &'static str = "mcp_servers::read";
    const DESCRIPTION: &'static str = "View MCP servers";
    const MODULE: &'static str = "mcp";
}

/// Permission to create user MCP servers
pub struct McpServersCreate;
impl PermissionCheck for McpServersCreate {
    const NAME: &'static str = "McpServersCreate";
    const PERMISSION: &'static str = "mcp_servers::create";
    const DESCRIPTION: &'static str = "Create MCP servers";
    const MODULE: &'static str = "mcp";
}

/// Permission to edit user MCP servers
pub struct McpServersEdit;
impl PermissionCheck for McpServersEdit {
    const NAME: &'static str = "McpServersEdit";
    const PERMISSION: &'static str = "mcp_servers::edit";
    const DESCRIPTION: &'static str = "Edit MCP servers";
    const MODULE: &'static str = "mcp";
}

/// Permission to delete user MCP servers
pub struct McpServersDelete;
impl PermissionCheck for McpServersDelete {
    const NAME: &'static str = "McpServersDelete";
    const PERMISSION: &'static str = "mcp_servers::delete";
    const DESCRIPTION: &'static str = "Delete MCP servers";
    const MODULE: &'static str = "mcp";
}

// =====================================================
// System MCP Server Permissions (Admin)
// =====================================================

/// Permission to view system MCP servers
pub struct McpServersAdminRead;
impl PermissionCheck for McpServersAdminRead {
    const NAME: &'static str = "McpServersAdminRead";
    const PERMISSION: &'static str = "mcp_servers_admin::read";
    const DESCRIPTION: &'static str = "View system MCP servers";
    const MODULE: &'static str = "mcp";
}

/// Permission to create system MCP servers
pub struct McpServersAdminCreate;
impl PermissionCheck for McpServersAdminCreate {
    const NAME: &'static str = "McpServersAdminCreate";
    const PERMISSION: &'static str = "mcp_servers_admin::create";
    const DESCRIPTION: &'static str = "Create system MCP servers";
    const MODULE: &'static str = "mcp";
}

/// Permission to edit system MCP servers
pub struct McpServersAdminEdit;
impl PermissionCheck for McpServersAdminEdit {
    const NAME: &'static str = "McpServersAdminEdit";
    const PERMISSION: &'static str = "mcp_servers_admin::edit";
    const DESCRIPTION: &'static str = "Edit system MCP servers and manage group assignments";
    const MODULE: &'static str = "mcp";
}

/// Permission to delete system MCP servers
pub struct McpServersAdminDelete;
impl PermissionCheck for McpServersAdminDelete {
    const NAME: &'static str = "McpServersAdminDelete";
    const PERMISSION: &'static str = "mcp_servers_admin::delete";
    const DESCRIPTION: &'static str = "Delete system MCP servers";
    const MODULE: &'static str = "mcp";
}

// =====================================================
// User-Policy Permissions (Admin)
// =====================================================

/// Permission to edit the global user MCP policy (allowed transports +
/// the sandbox flavor force-applied to user-installed stdio servers).
/// Read is open to any authenticated user — the UI needs the policy to
/// gate the Add button + Hub MCP tab visibility.
pub struct McpUserPolicyEdit;
impl PermissionCheck for McpUserPolicyEdit {
    const NAME: &'static str = "McpUserPolicyEdit";
    const PERMISSION: &'static str = "mcp_user_policy::edit";
    const DESCRIPTION: &'static str = "Edit MCP user policy (allowed transports + sandbox flavor)";
    const MODULE: &'static str = "mcp";
}

// =====================================================
// Helper Function to Collect All Permissions
// =====================================================
