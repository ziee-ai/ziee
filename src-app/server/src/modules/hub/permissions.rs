use crate::modules::permissions::PermissionCheck;

/// Read hub models permission
pub struct HubModelsRead;

impl PermissionCheck for HubModelsRead {
    const NAME: &'static str = "HubModelsRead";
    const PERMISSION: &'static str = "hub::models::read";
    const DESCRIPTION: &'static str = "View hub models";
    const MODULE: &'static str = "hub";
}

/// Read hub assistants permission
pub struct HubAssistantsRead;

impl PermissionCheck for HubAssistantsRead {
    const NAME: &'static str = "HubAssistantsRead";
    const PERMISSION: &'static str = "hub::assistants::read";
    const DESCRIPTION: &'static str = "View hub assistants";
    const MODULE: &'static str = "hub";
}

/// Read hub MCP servers permission
pub struct HubMCPServersRead;

impl PermissionCheck for HubMCPServersRead {
    const NAME: &'static str = "HubMCPServersRead";
    const PERMISSION: &'static str = "hub::mcp_servers::read";
    const DESCRIPTION: &'static str = "View hub MCP servers";
    const MODULE: &'static str = "hub";
}

/// Refresh hub models permission
pub struct HubModelsRefresh;

impl PermissionCheck for HubModelsRefresh {
    const NAME: &'static str = "HubModelsRefresh";
    const PERMISSION: &'static str = "hub::models::refresh";
    const DESCRIPTION: &'static str = "Refresh hub models from GitHub";
    const MODULE: &'static str = "hub";
}

/// Refresh hub assistants permission
pub struct HubAssistantsRefresh;

impl PermissionCheck for HubAssistantsRefresh {
    const NAME: &'static str = "HubAssistantsRefresh";
    const PERMISSION: &'static str = "hub::assistants::refresh";
    const DESCRIPTION: &'static str = "Refresh hub assistants from GitHub";
    const MODULE: &'static str = "hub";
}

/// Refresh hub MCP servers permission
pub struct HubMCPServersRefresh;

impl PermissionCheck for HubMCPServersRefresh {
    const NAME: &'static str = "HubMCPServersRefresh";
    const PERMISSION: &'static str = "hub::mcp_servers::refresh";
    const DESCRIPTION: &'static str = "Refresh hub MCP servers from GitHub";
    const MODULE: &'static str = "hub";
}

/// Read hub models version permission
pub struct HubModelsVersionRead;

impl PermissionCheck for HubModelsVersionRead {
    const NAME: &'static str = "HubModelsVersionRead";
    const PERMISSION: &'static str = "hub::models::read_version";
    const DESCRIPTION: &'static str = "View hub models version information";
    const MODULE: &'static str = "hub";
}

/// Read hub assistants version permission
pub struct HubAssistantsVersionRead;

impl PermissionCheck for HubAssistantsVersionRead {
    const NAME: &'static str = "HubAssistantsVersionRead";
    const PERMISSION: &'static str = "hub::assistants::read_version";
    const DESCRIPTION: &'static str = "View hub assistants version information";
    const MODULE: &'static str = "hub";
}

/// Read hub MCP servers version permission
pub struct HubMCPServersVersionRead;

impl PermissionCheck for HubMCPServersVersionRead {
    const NAME: &'static str = "HubMCPServersVersionRead";
    const PERMISSION: &'static str = "hub::mcp_servers::read_version";
    const DESCRIPTION: &'static str = "View hub MCP servers version information";
    const MODULE: &'static str = "hub";
}

/// Create assistants from hub permission
pub struct HubAssistantsCreate;

impl PermissionCheck for HubAssistantsCreate {
    const NAME: &'static str = "HubAssistantsCreate";
    const PERMISSION: &'static str = "hub::assistants::create";
    const DESCRIPTION: &'static str = "Create assistants from hub";
    const MODULE: &'static str = "hub";
}

/// Create MCP servers from hub permission
pub struct HubMcpServersCreate;

impl PermissionCheck for HubMcpServersCreate {
    const NAME: &'static str = "HubMcpServersCreate";
    const PERMISSION: &'static str = "hub::mcp_servers::create";
    const DESCRIPTION: &'static str = "Create MCP servers from hub";
    const MODULE: &'static str = "hub";
}

/// Download models from hub permission
pub struct HubModelsCreate;

impl PermissionCheck for HubModelsCreate {
    const NAME: &'static str = "HubModelsCreate";
    const PERMISSION: &'static str = "hub::models::download";
    const DESCRIPTION: &'static str = "Download models from hub";
    const MODULE: &'static str = "hub";
}

/// Read the unified hub catalog's admin views — list published
/// versions, list installed-items-behind-catalog. Read-only.
/// Follows the `module::resource::action` convention (mirrors
/// `code_sandbox::environments::read`).
pub struct HubCatalogRead;

impl PermissionCheck for HubCatalogRead {
    const NAME: &'static str = "HubCatalogRead";
    const PERMISSION: &'static str = "hub::catalog::read";
    const DESCRIPTION: &'static str = "View hub catalog versions + pending updates";
    const MODULE: &'static str = "hub";
}

/// Manage the unified hub catalog — force refresh + activate/pin a
/// catalog version server-wide. Administrators get this via the `*`
/// wildcard; nobody else does by default.
pub struct HubCatalogManage;

impl PermissionCheck for HubCatalogManage {
    const NAME: &'static str = "HubCatalogManage";
    const PERMISSION: &'static str = "hub::catalog::manage";
    const DESCRIPTION: &'static str = "Refresh + activate hub catalog versions";
    const MODULE: &'static str = "hub";
}
