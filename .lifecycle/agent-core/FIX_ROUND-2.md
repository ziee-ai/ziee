# FIX_ROUND-2 — re-audit of the Phase-7 fixes (blind, focused on the changed files)

## Fixed
- **R2.2 (maintainability, LOW)** — replaced the fn-level `#[allow(unused_variables)]` on `ChatToolProvider::call` with an `_idem` param rename, so a genuinely-dead local still warns.
- **comment error** — the `is_trusted` comment claimed `builtin_server_id_by_name` "INCLUDES control"; it maps only `code_sandbox` (+ the read-only set). Corrected.

## Confirmed correct by the re-audit (the Phase-7 fixes hold)
- `resolve_bare_tool_server` ambiguity guard: `return None` on the 2nd match + `matches.into_iter().next()` for the single-match case — correct.
- `is_trusted` SECURITY direction: `code_sandbox`→Some(id)→`is_builtin_server_id`=false→untrusted; `control`→None→untrusted. No mutating built-in auto-approved. Correct.
- `decide` claim-then-delete single-use + fail-closed denial — correct.

## Rejected / accepted with rationale
- **R2.1 (correctness, MED→LOW)** — REJECTED-preexisting-conservative. `is_trusted` false-denies 4 read-only built-ins (elicitation/knowledge_base/skill/tool_result) absent from the 7-entry `builtin_server_id_by_name`. This is PRE-EXISTING (before the Phase-7 fix `None.is_some()` also returned false) and CONSERVATIVE-SAFE (they route through review = more approvals, never a security hole; my fix only correctly flipped code_sandbox). Completing the name map is a UX-parity enhancement requiring precise verification of every built-in's server name + a `resolve_tool_server` fast-path regression — out of scope for this audit-convergence round; documented at the call site.
- **R2.3 (api-friendliness, LOW)** — ACCEPTED-latent. `#[must_use]` doesn't catch `let _ = AgentCoreFlag::on()`; all 6 callers use a named binding, so latent not live.

**New confirmed findings this round: 1** (R2.2 fixed; R2.1 rejected-preexisting, R2.3 accepted-latent).
