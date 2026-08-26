# FIX_ROUND-1 — code-sandbox-chat-attach

Blind audit round 1 ran two angles differing in kind: **security** + **design-conformance**
(fresh/blind subagents, diff-only context). Full findings in `LEDGER.jsonl`.

## Findings triaged + resolved

- **security / MED — AutoApprove asymmetry** (confirmed → fixed via ITEM-5).
  `execute_command` (arbitrary code) took the regular-tool approval path, so under
  `ApprovalMode::AutoApprove` it auto-ran with no prompt — unlike control/background
  which force approval even under AutoApprove. Severity-security ⇒ entered the fix
  loop regardless of corroboration. Fixed: added `code_sandbox_call_needs_approval`
  (handlers.rs) + the `is_code_sandbox` arm in the approval ladder (mcp.rs). Now
  execute_command / write_file / edit_file always require approval; the read-only
  tools auto-run. Covered by TEST-6. Reinforces INV-2.

- **design-conformance / MED — enabled producer path never exercised** (confirmed →
  fixed via TEST-7). The enabled→advertised producer wiring was only unit-tested
  piecewise; a producer that gated wrong or forgot to call apply would pass all
  tests. Fixed: extracted `apply_attach_if_eligible` (the pure form of
  `before_llm_call`'s glue) and added TEST-7 asserting the flag is SET when eligible
  and unset otherwise (rootfs-free). Closes the positive INV-1 coverage gap.

- **design-conformance / LOW — doc imprecision** (confirmed → fixed). Corrected the
  enabled-predicate comment from "enabled AND init reached Ready" to "enabled AND
  workspace init succeeded" (STATE is set before the Ready status is stamped).

- **design-conformance / LOW — single-gate vs siblings' double-gate** (rejected →
  wontfix). Deliberate: code_sandbox gates solely on `get_state().is_some()` at the
  producer (matches `mount_context_extension`), proven by the disabled-path e2e.
  Informational; no change.

## Self-review of the fix diff

The fix touches: chat_extension/mod.rs (new pure helper + comment + TEST-7),
handlers.rs (new classifier), mcp.rs (is_code_sandbox arm + TEST-6). The
`is_code_sandbox` arm is byte-shaped like the pre-existing `is_control`/`is_background`
arms; the classifier mirrors `background_call_needs_approval`. No new finding: the
change is additive and the security-critical non-membership in `is_builtin_server_id`
is unchanged (still asserted by TEST-4 + the pre-existing matrix test).

LIGHT tier → one audit round; round complete, all confirmed findings resolved.

**New confirmed findings:** 0
