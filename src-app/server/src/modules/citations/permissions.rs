//! Permission keys for the citations module.

use crate::modules::permissions::types::PermissionCheck;

/// Use the built-in citation tools (lookup / add / verify / list / format /
/// remove) and read your own library. Granted to the default Users group by
/// migration 104.
///
/// NOTE on the use/manage split: the MCP `tools/call` surface gates the WHOLE
/// endpoint (including the mutating `add_citations`/`remove_citations` tools) on
/// `CitationsUse`, while the REST mutation endpoints require `CitationsManage`.
/// This asymmetry is intentional — the library is strictly per-user (every query
/// is `WHERE user_id = $1`), so a model acting on the user's behalf writing the
/// user's OWN library is the same trust level as reading it; there is no
/// cross-tenant exposure. Project-list mutations are additionally guarded by an
/// explicit project-ownership check regardless of transport.
pub struct CitationsUse;
impl PermissionCheck for CitationsUse {
    const NAME: &'static str = "CitationsUse";
    const PERMISSION: &'static str = "citations::use";
    const DESCRIPTION: &'static str = "Use the citation tools and read your bibliography library.";
    const MODULE: &'static str = "citations";
}

/// Mutate the bibliography library: import, add, remove, verify, enrich, manage
/// project reference lists, and upload CSL styles. The library is per-user, so
/// normal users hold this for their own data (granted by migration 104).
pub struct CitationsManage;
impl PermissionCheck for CitationsManage {
    const NAME: &'static str = "CitationsManage";
    const PERMISSION: &'static str = "citations::manage";
    const DESCRIPTION: &'static str =
        "Create, import, verify, remove, and organize citations + CSL styles.";
    const MODULE: &'static str = "citations";
}
