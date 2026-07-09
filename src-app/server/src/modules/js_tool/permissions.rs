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
