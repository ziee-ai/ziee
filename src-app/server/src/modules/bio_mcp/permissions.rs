//! Permission keys for the bio_mcp module.

use crate::modules::permissions::types::PermissionCheck;

/// Query the built-in BioMCP server (call its tools via the proxy route).
/// Granted to the default Users group by migration, mirroring how
/// `memory::read` gates the memory MCP route — so JWT auth runs on every
/// `/api/bio/mcp` call and admins can revoke biomedical access per group.
pub struct BioQuery;
impl PermissionCheck for BioQuery {
    const NAME: &'static str = "BioQuery";
    const PERMISSION: &'static str = "bio::query";
    const DESCRIPTION: &'static str = "Query the built-in BioMCP biomedical tools.";
    // Logical feature name "bio" (matches the `bio::` permission prefix, the
    // server row name 'bio', and the `bio.ziee.internal` UUID) — mirrors how
    // memory uses MODULE="memory" / `memory::read`, not the bridge dir name.
    const MODULE: &'static str = "bio";
}
