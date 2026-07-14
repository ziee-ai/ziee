# DRIFT-1 — implementation vs plan

Audited the implemented diff against PLAN.md item by item, after implementation and
before any test-suite run.

## Per-item reconciliation

- **DRIFT-1.1** — verdict: impl-wins — **PLAN said** ITEM-4 changes the delete at `mcp.rs:1074` (one site). **Impl** hoists the claim to the top of the loop body AND removes THREE further `delete_tool_approval` calls (the server-not-found, sampling-no-session, and connect-fail arms), each of which `continue`d before the post-execution delete and so carried its own copy. Once the row is claimed up front they are dead double-deletes. The plan under-counted the delete sites because recon only examined the post-execution one. Keeping them would leave three redundant DB round-trips and, worse, imply each new error arm must remember to re-add a delete — the anti-spin property should be structural. PLAN.md ITEM-4 amended to say "the loop's single claim point"; TESTS.md unchanged (TEST-10 already asserts the row is gone, and the pre-existing `mcp_approval_loop_unresolvable_tool_errors_and_terminates` covers the unresolvable arm — it now depends solely on the claim, making it a free regression guard for this removal).
- **DRIFT-1.2** — verdict: impl-wins — **PLAN/TESTS said** TEST-1 "must fail on current code". **Impl** cannot literally satisfy that: TEST-1 calls `replace_or_collect_tool_results`, which does not exist pre-fix, so the test cannot be run against pre-fix code at all. Rather than let that claim stand hollow (a paper-fails-pre-fix), TEST-1 now embeds an explicit CONTROL block that performs the PRE-FIX behavior (blind-append) on the same shape and asserts it really does yield `["A","B","B"]` and that the invariant assertion really does catch it. The test therefore proves both that the bug shape is real and that the fix removes it. TESTS.md TEST-1 wording amended to describe the control instead of the impossible claim.
- **DRIFT-1.3** — verdict: resolved — **PLAN said** `dedup_tool_results_by_id` is a "new pure fn"; **impl** made it `pub` (not private) because ITEM-7 re-exports it through `test_internals` for TEST-9. Consistent with `group_assistant_blocks`, which is `pub` for exactly the same reason. No behavior change; the plan's intent (a pure, unit-testable fn) is met.
- **DRIFT-1.4** — verdict: resolved — **PLAN said** TEST-12 asserts via `include_str!(file!())`. `file!()` is crate-root-relative while `include_str!` is file-relative, so that spelling does not compile. Impl uses `include_str!("contents.rs")` (self-include) and scans only the pre-`mod tests` section — copying the established precedent `code_sandbox::backend::wsl2::med3_wslenv_credential_leak_regression`, which the plan cited as the model. Mechanism corrected, assertion identical.
- **DRIFT-1.5** — verdict: impl-wins — **TESTS.md TEST-10 said** it asserts the row is deleted BEFORE the tool executes. A DB-failure-ordering claim is not directly observable through the HTTP/mock harness without fault injection, and the mock has no closure handler to query the DB mid-call. **Impl** asserts the observable consequences that ITEM-4 actually guarantees: (a) nothing executes before approval, (b) the row is gone afterward, (c) the mock received EXACTLY ONE `tools/call`, (d) exactly ONE `tool_result` row is persisted. Honest framing recorded in TESTS.md: TEST-10 is an exactly-once + no-duplicate-row guard (and the guard for DRIFT-1.1's removals), not a fails-pre-fix test. The ordering itself IS proven indirectly by `mcp_approval_loop_unresolvable_tool_errors_and_terminates`: that arm returns BEFORE any execution, and its row is still deleted — only possible if the claim precedes execution.
- **ITEM-1** — verdict: none — implemented as planned (free fn, in-place replace, leftover-vec return, wired at the `mcp.rs` push site).
- **ITEM-2** — verdict: none — implemented as planned (keep-first, `warn!`, empties removed, called at `streaming.rs:468` before `clear_old_tool_results`).
- **ITEM-3** — verdict: none — `results_by_id.clear()` at the end of `flush_assistant_tool_pair`, as planned. Verified to genuinely fail pre-fix by temporarily reverting the line (TEST-5 panicked with "X must carry its REAL result"), then restoring.
- **ITEM-5** — verdict: none — comment corrected; no code touched.
- **ITEM-6** — verdict: none — migration 158 added as planned, mirroring 124's header style.
- **ITEM-7** — verdict: none — `dedup_tool_results_by_id` re-exported through `test_internals`.

## Scope check (no silent additions)

Diff touches exactly the 6 planned files plus the 3 planned test files, and nothing
else. No unplanned behavior rode along:
- `clear_old_tool_results` — untouched (exonerated in PLAN).
- `contents.rs` `append_content` body — untouched (comment only); the parallel-tool
  ordering fix is intact.
- No `openapi.json` / `api-client/types.ts` change, matching BASE.md's prediction.

## One deviation NOT taken (recorded so it is a decision, not an oversight)

TEST-2's `text(...)` fixture: `mod tests` in `streaming.rs` already had a `text()`
builder, so the new dedup tests reuse it rather than adding a parallel one — matching
PLAN's "Patterns to follow" rule about not inventing a second fixture vocabulary.

**Unresolved drifts:** 0
