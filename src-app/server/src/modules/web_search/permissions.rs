//! Permission keys for the web_search module.

use crate::modules::permissions::types::PermissionCheck;

/// Use the built-in web search + page-fetch tools (`web_search` / `fetch_url`).
/// Granted to the default Users group by migration 97.
pub struct WebSearchUse;
impl PermissionCheck for WebSearchUse {
    const NAME: &'static str = "WebSearchUse";
    const PERMISSION: &'static str = "web_search::use";
    const DESCRIPTION: &'static str = "Use the web search + page-fetch tools.";
    const MODULE: &'static str = "web_search";
}

/// Read deployment-wide web search settings + provider catalog.
pub struct WebSearchAdminRead;
impl PermissionCheck for WebSearchAdminRead {
    const NAME: &'static str = "WebSearchAdminRead";
    const PERMISSION: &'static str = "web_search::admin::read";
    const DESCRIPTION: &'static str = "Read web search settings (enable, provider chain, caps).";
    const MODULE: &'static str = "web_search";
}

/// Mutate deployment-wide web search settings + provider config/keys.
pub struct WebSearchAdminManage;
impl PermissionCheck for WebSearchAdminManage {
    const NAME: &'static str = "WebSearchAdminManage";
    const PERMISSION: &'static str = "web_search::admin::manage";
    const DESCRIPTION: &'static str =
        "Update web search settings, provider chain, and provider API keys.";
    const MODULE: &'static str = "web_search";
}
