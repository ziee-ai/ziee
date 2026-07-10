//! Permission key for the js_tool (`run_js`) module.

use crate::modules::permissions::types::PermissionCheck;

/// Use the built-in `run_js` programmatic-tool-calling tool. Granted to the
/// default Users group by migration 134.
///
/// `run_js` only exposes tools the conversation already has, runs them in an
/// embedded interpreter with zero ambient capability, and routes every gated /
/// mutating sub-tool through the same per-call approval the normal loop uses —
/// so it is the same trust level as the model's existing tool access.
pub struct JsToolUse;
impl PermissionCheck for JsToolUse {
    const NAME: &'static str = "JsToolUse";
    const PERMISSION: &'static str = "js_tool::use";
    const DESCRIPTION: &'static str =
        "Use the built-in run_js tool (programmatic tool calling in an embedded JS runtime).";
    const MODULE: &'static str = "js_tool";
}

/// Read the admin-configurable run_js limits (js_tool_settings). Admin-only —
/// held via the Administrators `*` wildcard; NOT granted to the Users group.
pub struct JsToolSettingsRead;
impl PermissionCheck for JsToolSettingsRead {
    const NAME: &'static str = "JsToolSettingsRead";
    const PERMISSION: &'static str = "js_tool::settings::read";
    const DESCRIPTION: &'static str = "Read the run_js (js_tool) resource-limits configuration.";
    const MODULE: &'static str = "js_tool";
}

/// Update the admin-configurable run_js limits (memory/stack/wall/approval/
/// concurrency/trace caps). Admin-only — held via `*`.
pub struct JsToolSettingsManage;
impl PermissionCheck for JsToolSettingsManage {
    const NAME: &'static str = "JsToolSettingsManage";
    const PERMISSION: &'static str = "js_tool::settings::manage";
    const DESCRIPTION: &'static str =
        "Update the run_js (js_tool) memory/stack/wall-clock/approval-timeout/concurrency/trace caps.";
    const MODULE: &'static str = "js_tool";
}
