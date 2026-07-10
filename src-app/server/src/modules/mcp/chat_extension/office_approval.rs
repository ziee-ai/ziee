//! Per-call MCP tool-approval decision (extracted from the approval loop in
//! `mcp.rs`) + the `office_bridge` `run_office_js` read-mode bypass.
//!
//! Mirrors `control_mcp`'s posture: `office_bridge` is auto-attached but NOT in the
//! approval-bypass set, so its tools require approval by default — EXCEPT a
//! `run_office_js` call the model declares `mode:"read"`, which auto-runs. A
//! `write` (or a missing / non-`"read"` mode, or any other tool) falls through to the
//! normal per-conversation ManualApprove flow (prompt, or auto-run if the user picked
//! "always allow"). The model is trusted to declare `mode` honestly — there is NO
//! read-only enforcement (see the office-mode-gated-approval lifecycle decisions).

use serde_json::Value;
use uuid::Uuid;

use crate::modules::mcp::chat_extension::ApprovalMode;

/// The deterministic id of the built-in `office_bridge` MCP server row. Recomputed
/// here (the server lib cannot depend on the desktop crate) from the SAME string the
/// desktop `office_bridge::mod.rs` uses; a desktop-crate test asserts the two match so
/// they can never drift.
pub fn office_bridge_mcp_server_id() -> Uuid {
    Uuid::new_v5(&Uuid::NAMESPACE_URL, b"office_bridge.ziee.internal")
}

/// True IFF this is an `office_bridge` `run_office_js` call the model declared as a
/// read: server id == office_bridge, tool == `run_office_js`, and `input.mode` is the
/// EXACT string `"read"`. Everything else — a `write`, a missing / null / non-string /
/// any-other-string `mode`, a different tool, or a non-office server that merely names
/// a tool `run_office_js` — is NOT a read bypass (fail-safe + spoof-safe).
pub fn run_office_js_read_bypass(server_id: Option<Uuid>, tool_name: &str, input: &Value) -> bool {
    server_id == Some(office_bridge_mcp_server_id())
        && tool_name == "run_office_js"
        && input.get("mode").and_then(Value::as_str) == Some("read")
}

/// Pure per-call approval decision — returns whether the tool call must be routed
/// through user approval (`true`) or may execute immediately (`false`). Extracted
/// verbatim from the `mcp.rs` approval loop (behaviour-preserving for control /
/// builtin / disabled / manual / auto-approved servers) with ONE added branch: the
/// office_bridge `run_office_js` read bypass.
///
/// `is_auto_approved` is precomputed by the caller (the per-conversation +
/// per-user `auto_approved_servers` `contains_tool` check) so this stays DB-free.
pub fn compute_needs_approval(
    server_id: Option<Uuid>,
    tool_name: &str,
    input: &Value,
    approval_mode: ApprovalMode,
    is_builtin: bool,
    is_control: bool,
    is_auto_approved: bool,
) -> bool {
    // Control: read-only control tools auto-run; mutating `invoke_capability` always
    // approves (overriding even AutoApprove) — unchanged.
    if is_control {
        return crate::modules::control_mcp::handlers::control_call_needs_approval(tool_name, input);
    }
    // Privileged built-ins bypass approval entirely — unchanged.
    if is_builtin {
        return false;
    }
    // Office read bypass: only an exact `run_office_js` `mode:"read"` on office_bridge.
    // Anything else on office_bridge (write / missing mode / list_open_documents) falls
    // through to the normal path below (write → prompt, or auto-run if always-allowed).
    if run_office_js_read_bypass(server_id, tool_name, input) {
        return false;
    }
    match approval_mode {
        ApprovalMode::AutoApprove => false,
        ApprovalMode::ManualApprove => !is_auto_approved,
        // The caller denies Disabled + non-builtin before this; treat as needs-approval
        // (never silently auto-run) for total-function safety.
        ApprovalMode::Disabled => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn office() -> Option<Uuid> {
        Some(office_bridge_mcp_server_id())
    }

    // ── TEST-10: run_office_js_read_bypass — the full read-bypass matrix ──────────

    #[test]
    fn read_bypass_only_for_exact_office_run_office_js_read() {
        // The one true case.
        assert!(run_office_js_read_bypass(
            office(),
            "run_office_js",
            &json!({ "mode": "read" })
        ));
    }

    #[test]
    fn write_and_missing_and_fuzzy_mode_never_bypass() {
        for input in [
            json!({ "mode": "write" }),
            json!({}),                       // missing
            json!({ "mode": serde_json::Value::Null }),
            json!({ "mode": 1 }),            // non-string
            json!({ "mode": "READ" }),       // wrong case
            json!({ "mode": "Read" }),
            json!({ "mode": "read " }),      // trailing space
            json!({ "mode": "readonly" }),
        ] {
            assert!(
                !run_office_js_read_bypass(office(), "run_office_js", &input),
                "must NOT bypass for input {input}"
            );
        }
    }

    #[test]
    fn a_different_office_tool_never_bypasses_even_read() {
        assert!(!run_office_js_read_bypass(
            office(),
            "list_open_documents",
            &json!({ "mode": "read" })
        ));
    }

    #[test]
    fn a_non_office_server_spoofing_run_office_js_never_bypasses() {
        let not_office = Some(Uuid::new_v5(&Uuid::NAMESPACE_URL, b"some.other.server"));
        assert_ne!(not_office, office());
        assert!(!run_office_js_read_bypass(
            not_office,
            "run_office_js",
            &json!({ "mode": "read" })
        ));
        // Unparseable / absent server id also never bypasses.
        assert!(!run_office_js_read_bypass(None, "run_office_js", &json!({ "mode": "read" })));
    }

    // ── TEST-12: compute_needs_approval — every branch, behaviour-preserving ─────

    // Helper: a normal (non-office, non-control, non-builtin) server id.
    fn normal() -> Option<Uuid> {
        Some(Uuid::new_v5(&Uuid::NAMESPACE_URL, b"normal.server"))
    }
    // Convenience: compute for a normal server.
    fn decide(mode: ApprovalMode, is_auto: bool) -> bool {
        compute_needs_approval(normal(), "some_tool", &json!({}), mode, false, false, is_auto)
    }

    #[test]
    fn builtin_bypasses() {
        // is_builtin short-circuits to false regardless of mode/auto.
        assert!(!compute_needs_approval(
            normal(), "any", &json!({}), ApprovalMode::ManualApprove, true, false, false
        ));
    }

    #[test]
    fn control_delegates_to_control_classifier() {
        // A read-only control tool → auto-run; delegation is via is_control=true.
        assert!(!compute_needs_approval(
            Some(crate::modules::control_mcp::control_mcp_server_id()),
            "list_capabilities",
            &json!({}),
            ApprovalMode::ManualApprove,
            false,
            true,
            false,
        ));
    }

    #[test]
    fn office_read_bypasses_office_write_prompts() {
        // office_bridge run_office_js read → auto-run.
        assert!(!compute_needs_approval(
            office(), "run_office_js", &json!({ "mode": "read" }),
            ApprovalMode::ManualApprove, false, false, false
        ));
        // office_bridge run_office_js write (not auto-approved) → prompt.
        assert!(compute_needs_approval(
            office(), "run_office_js", &json!({ "mode": "write" }),
            ApprovalMode::ManualApprove, false, false, false
        ));
        // office_bridge run_office_js write + always-allowed → auto-run.
        assert!(!compute_needs_approval(
            office(), "run_office_js", &json!({ "mode": "write" }),
            ApprovalMode::ManualApprove, false, false, true
        ));
        // office_bridge run_office_js missing mode → treated as write → prompt (fail-safe).
        assert!(compute_needs_approval(
            office(), "run_office_js", &json!({}),
            ApprovalMode::ManualApprove, false, false, false
        ));
    }

    #[test]
    fn normal_server_manual_vs_auto_approved_vs_auto_mode() {
        // ManualApprove, not auto-approved → prompt.
        assert!(decide(ApprovalMode::ManualApprove, false));
        // ManualApprove, auto-approved (always-allow) → auto-run.
        assert!(!decide(ApprovalMode::ManualApprove, true));
        // AutoApprove mode → auto-run.
        assert!(!decide(ApprovalMode::AutoApprove, false));
    }
}
