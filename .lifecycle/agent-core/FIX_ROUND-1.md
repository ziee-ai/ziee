# FIX_ROUND-1 — Phase-7 fixes on the blind Phase-6 audit

## Fixed (confirmed findings)
- **A3.2 (security, HIGH)** — workflow agent host `is_trusted` auto-trusted `code_sandbox`/`control` (via `builtin_server_id_by_name`) → a `kind:agent` step's sandbox code executed with NO reviewer/gate. Fixed to resolve name→id then gate on `is_builtin_server_id` (the read-only bypass set that EXCLUDES them), matching the chat twin. `workflow/agent_dispatch.rs`.
- **A2.2/A3.1 (correctness, MED)** — `resolve_bare_tool_server` guessed the FIRST server advertising a bare tool name; legacy never guesses. Fixed: collect all matches, return `None` if >1 (ambiguity guard, parity with `recover_server_id_for_bare_name`). `chat/agent_host/gate.rs`.
- **A3.6 (tests-reality, MED)** — `model_resolver_test` never set the flag → tested the legacy path. Fixed: run under `AgentCoreFlag::on()` (exercises the agent-core dispatcher's model gate) + corrected the mis-attributed header.
- **A3.7 (tests-quality, MED)** — process-global `set_var`/`remove_var` leaked the flag ON on a panic + under `--test-threads>1`. Fixed: panic-safe RAII `AgentCoreFlag` guard in `tests/common/mod.rs`, applied to all 5 flag-setting tests.
- **A3.8 (tests-reality, MED)** — multiturn test asserted `history.contains('purple-turtle-42')` (already written by turn-1). Fixed: reconstruct turn-2's OWN streamed response text and assert the recall there (fails if cross-turn recall breaks).
- **A1.4 (api-friendliness, LOW)** — chat passed `Some(idem)` that `call_mcp_tool` silently drops. Fixed: pass `None` + document why.
- **A1.3 (maintainability, MED)** — `mod.rs` "integration contract" was wrong (claimed `ChatToolProvider::list` from NAMEs is the chat tool source; it's overwritten by the extension's `before_llm_call`, uuid scheme). Fixed the doc.

## Confirmed but ACCEPTED with rationale (not silent dismissals)
- **A1.1 (modularity, MED)** — `call_mcp_tool` lives in the workflow module (chat imports it). This coupling is **pre-existing** (call_mcp_tool was shared in workflow before this branch; the refactor only added the `ChatCallCtx` param, which is well-documented as the chat-context). Moving the chokepoint to a neutral `mcp` module is a legitimate follow-up but re-churns regression-locked shared code; recorded as a known item, not fixed this round to preserve the byte-identical baseline.
- **A1.2 (api-friendliness, MED)** — two adjacent `Option<String>` (`review_classification`, `idempotency_key`) are swappable. Both predate the refactor; only 2 correct workflow call sites pass them (chat now passes `None`/`None`); they're documented at the def. A struct/newtype refactor re-churns the regression-verified shared signature for a low-probability footgun; accepted with the doc + the A1.4 clarification.
- **A2.1 (perf, LOW)** — `decide()` runs 2 indexed `branch_id` queries per tool-decision; required for cross-request resume-claim correctness (each decision must check for a prior approval). Accepted (indexed, only on tool-calling turns).
- **A2.4 (state-management, LOW-MED)** — a stopped mid-stream generation isn't persisted; needs a legacy-parity check (if legacy also drops a cancelled partial it's parity, not a regression). Deferred pending that check.
- **A2.3 (concurrency, LOW)** — fan_out sibling-cancel-on-error; fanout is DESCOPED (no production caller), dead path.
- **A3.3/A3.4 (security, LOW)** — no concrete escalation; the downstream loopback-JWT re-check is the real per-tool gate (covered by existing bio/web_search perm tests). Follow-up: an explicit "permission-lacking user denied via agent path" test.
- **A3.9/A3.10 (tests-quality, LOW)** — weak-but-deterministic assertions with a stronger sibling anchor (count==0 / the constraint exists); accepted.

## Rejected
- ports.rs "trait leak" — REJECTED (clean; domain-neutral).
- verification substring — the `count==0` bibliography check is the real anchor (rejected-minor).

**New confirmed findings this round: 13** (7 fixed, 6 accepted-with-rationale/deferred, per above).
