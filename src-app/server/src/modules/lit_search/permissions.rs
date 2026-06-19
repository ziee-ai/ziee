//! Permission keys for the lit_search module.

use crate::modules::permissions::types::PermissionCheck;

/// Use the built-in literature search + full-text tools (`literature_search` /
/// `fetch_paper_fulltext`). Granted to the default Users group by migration 101.
pub struct LitSearchUse;
impl PermissionCheck for LitSearchUse {
    const NAME: &'static str = "LitSearchUse";
    const PERMISSION: &'static str = "lit_search::use";
    const DESCRIPTION: &'static str = "Use the literature search + full-text tools.";
    const MODULE: &'static str = "lit_search";
}

/// Read deployment-wide lit_search settings + connector catalog.
pub struct LitSearchAdminRead;
impl PermissionCheck for LitSearchAdminRead {
    const NAME: &'static str = "LitSearchAdminRead";
    const PERMISSION: &'static str = "lit_search::admin::read";
    const DESCRIPTION: &'static str =
        "Read literature search settings (enable, active sources, caps).";
    const MODULE: &'static str = "lit_search";
}

/// Mutate deployment-wide lit_search settings + connector config/keys.
pub struct LitSearchAdminManage;
impl PermissionCheck for LitSearchAdminManage {
    const NAME: &'static str = "LitSearchAdminManage";
    const PERMISSION: &'static str = "lit_search::admin::manage";
    const DESCRIPTION: &'static str =
        "Update literature search settings, active sources, and source API keys.";
    const MODULE: &'static str = "lit_search";
}
