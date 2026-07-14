# TESTS — fix-duplicate-tool-result

Every ITEM is covered by ≥1 TEST. Backend-only diff (`BASE.md`): no frontend path is touched, so
no `tier: e2e` test is required and no new permission exists (A9/A10 do not apply).

**The headline test is TEST-1** — it reproduces the EXACT assembled shape through the real
production functions and MUST FAIL on current code. Everything else is supporting coverage.

## Tests

- **TEST-1** (tier: unit) [covers: ITEM-1, ITEM-2] file: `src-app/server/src/modules/mcp/chat_extension/mcp.rs` — asserts: THE REPRO. Feed the real mixed-batch shape — `group_assistant_blocks([tool_use A, tool_use B, tool_result A])` yields `[Assistant{A,B}, Tool{result A, SYNTHETIC result B}]` — then apply `replace_or_collect_tool_results` with B's freshly-executed real result. Asserts (a) no `tool_use_id` appears in more than one `tool_result` across the whole request, (b) B's surviving block is the REAL result, not the `is_error` placeholder, (c) every `tool_use` is still resolved in the message immediately after its Assistant turn. **Amended per DRIFT-1.2:** the test cannot literally "fail pre-fix" (it calls a fn that does not exist pre-fix), so it embeds an explicit CONTROL that performs the PRE-FIX blind-append on the same shape and asserts it really does produce `["A","B","B"]` AND that the invariant assertion really catches it — proving the bug shape is real and the test is not vacuous.
- **TEST-2** (tier: unit) [covers: ITEM-2] file: `src-app/server/src/modules/chat/core/services/streaming.rs` — asserts: `dedup_tool_results_by_id` keeps the FIRST `tool_result` for a repeated `tool_use_id` and drops later ones; exactly one result per id survives; non-duplicate blocks keep their relative order.
- **TEST-3** (tier: unit) [covers: ITEM-2] file: `src-app/server/src/modules/chat/core/services/streaming.rs` — asserts: `dedup_tool_results_by_id` is a no-op on an already-valid request (message count, block count and order all unchanged) — the defense never perturbs the healthy path.
- **TEST-4** (tier: unit) [covers: ITEM-2] file: `src-app/server/src/modules/chat/core/services/streaming.rs` — asserts: a message whose ONLY block is dropped as a duplicate is removed entirely from the request (no empty `content` array, which Anthropic rejects).
- **TEST-5** (tier: unit) [covers: ITEM-3] file: `src-app/server/src/modules/chat/core/services/streaming.rs` — asserts: after a flush, a stale orphan result no longer shadows a later REAL result for the same id — `[result X(orphan), use A, result A, use X, result X(real)]` gives X the REAL result. Fails pre-fix (`or_insert` keeps the orphan).
- **TEST-6** (tier: unit) [covers: ITEM-1] file: `src-app/server/src/modules/mcp/chat_extension/mcp.rs` — asserts: `replace_or_collect_tool_results` REPLACES an existing placeholder for B in place (same message, same index — adjacency to its tool_use preserved) and returns an EMPTY leftover vec, so no User message is pushed.
- **TEST-7** (tier: unit) [covers: ITEM-1] file: `src-app/server/src/modules/mcp/chat_extension/mcp.rs` — asserts: with NO existing block for the id (the pure awaiting-approval batch — bare Assistant turn, no Tool message), the result is returned as a leftover for the User message and the request is left untouched. This is the `chat-toolresult-pairing` regression guard.
- **TEST-8** (tier: unit) [covers: ITEM-1] file: `src-app/server/src/modules/mcp/chat_extension/mcp.rs` — asserts: a MIXED fresh batch — one id with an existing placeholder, one without — replaces the first in place and returns only the second as a leftover.
- **TEST-9** (tier: integration) [covers: ITEM-2, ITEM-7] file: `src-app/server/tests/chat/assistant_block_grouping_test.rs` — asserts: via the `ziee::test_internals` bridge, a request-level invariant `assert_single_result_per_tool_use` holds on the resume shape after `dedup_tool_results_by_id`; extends the existing `assert_valid_tool_pairing` so both invariants (exactly-one-result AND immediately-after pairing) are proven together.
- **TEST-10** (tier: integration) [covers: ITEM-4] file: `src-app/server/tests/mcp/approval_claim_test.rs` — asserts: exactly-once execution via the REAL path (stub LLM → finalize → MCP extension → approval → resume) against an in-process MCP mock: (a) nothing executes before approval, (b) the approval row is GONE after the run (claimed), (c) the mock received EXACTLY ONE `tools/call`, (d) exactly ONE `tool_result` row is persisted for that `tool_use_id`. **Honest scope per DRIFT-1.5:** this is an exactly-once + no-duplicate-row guard (and the regression guard for DRIFT-1.1's removal of three redundant deletes), NOT a fails-pre-fix test — a DB-DELETE-failure ordering claim needs fault injection the harness lacks. The claim-BEFORE-execute ordering is proven indirectly by the pre-existing `mcp_approval_loop_unresolvable_tool_errors_and_terminates`: that arm returns before any execution, yet its row is still deleted — only possible if the claim precedes execution.
- **TEST-11** (tier: integration) [covers: ITEM-6] file: `src-app/server/tests/chat/append_content_ordering_test.rs` — asserts: after migration 158, `message_contents` carries EXACTLY ONE unique index on `(message_id, sequence_order)` (query `pg_indexes`/`pg_constraint`), the redundant `idx_message_contents_message_seq_unique` is gone, and the surviving `uq_message_contents_message_sequence` still rejects a colliding `sequence_order` — the protection is preserved, only the duplicate removed.
- **TEST-12** (tier: unit) [covers: ITEM-5] file: `src-app/server/src/modules/chat/core/repository/contents.rs` — asserts: doc-comment accuracy guard — the `append_content` header does NOT claim the UNIQUE constraint is still "the next step" (a `#[test]` over `include_str!(file!())` asserting the stale phrasing is absent). Mirrors the existing docstring-accuracy check added by the sibling `chat-toolresult-pairing` fix (commit `51b5928a8`, "docstring accuracy").

## Added in FIX_ROUND-1 (blind-audit findings — no prior TEST-ID may be dropped, A5)

- **TEST-13** (tier: unit) [covers: ITEM-4] file: `src-app/server/src/modules/mcp/chat_extension/mcp.rs` — asserts: the claim VERDICT. `claim_outcome(Ok(true)) == Won` (we own the execution), `claim_outcome(Ok(false)) == AlreadyClaimed` (a concurrent pass claimed it — MUST NOT execute), `claim_outcome(Err) == Failed` (fail loudly). This is the leg that DISCRIMINATES the fix: branching only on `Err` — discarding `delete_tool_approval`'s `Ok(rows_affected > 0)` — silently turns AlreadyClaimed into Won, i.e. a double-run. TEST-10 cannot discriminate because the bug needs a losing/failing DELETE the HTTP harness cannot induce.
- **TEST-14** (tier: unit) [covers: ITEM-2] file: `src-app/server/src/modules/chat/core/services/streaming.rs` — asserts: a `tool_use_id` reused across TURNS (gpt-oss/harmony streams the constant `"tool_use"`; `resolve_unique_tool_use_id` scopes uniqueness per `message_id`) is NOT a duplicate — both turns keep their own result and nothing is dropped. **Verified to fail on the pre-fix (global-scope) code**: turn 2's whole Tool message was deleted (`left: 3, right: 4`), which would have unpaired its tool_use and 400'd every gpt-oss conversation.
- **TEST-15** (tier: unit) [covers: ITEM-1] file: `src-app/server/src/modules/mcp/chat_extension/mcp.rs` — asserts: `replace_or_collect_tool_results` ignores the same id in an OLDER turn — it must not overwrite that turn's result (history corruption) nor swallow the leftover, which would leave the CURRENT tool_use unanswered.
- **TEST-16** (tier: unit) [covers: ITEM-3] file: `src-app/server/src/modules/chat/core/services/streaming.rs` — asserts: the orphan half that `results_by_id.clear()` could NOT close — `[result X(stale), use X, result X(real)]` never flushes in between, so only refusing to capture a result that answers no OUTSTANDING tool_use fixes it. X must carry its real result.
- **TEST-17** (tier: unit) [covers: ITEM-1, ITEM-2] file: `src-app/server/src/modules/mcp/chat_extension/mcp.rs` — asserts: `#[should_panic]` — the `assert_single_result_per_tool_use` invariant helper actually CATCHES a duplicated id, proving the assertions built on it (incl. TEST-1's control) are not vacuous. Replaces a `catch_unwind` control that swapped the process-global panic hook and would have swallowed parallel tests' panic output.

## Coverage map (bipartite check)

| ITEM | Covered by |
|---|---|
| ITEM-1 | TEST-1, TEST-6, TEST-7, TEST-8, TEST-15, TEST-17 |
| ITEM-2 | TEST-1, TEST-2, TEST-3, TEST-4, TEST-9, TEST-14, TEST-17 |
| ITEM-3 | TEST-5, TEST-16 |
| ITEM-4 | TEST-10, TEST-13 |
| ITEM-5 | TEST-12 |
| ITEM-6 | TEST-11 |
| ITEM-7 | TEST-9 |

## Regression guards (must stay green, not new tests)

- `streaming.rs` `group_assistant_blocks_*` (`:2259-2423`) — the sibling `chat-toolresult-pairing`
  fix. ITEM-3 changes that fn; TEST-8 (`group_assistant_blocks_dedups_duplicate_result`) is the
  pin.
- `mod trim_tests` (`:2504-2697`) — `clear_old_tool_results` is exonerated and untouched; these
  prove ITEM-2's dedup (which runs immediately before it) didn't disturb the keep-last-K window.
- `tests/chat/assistant_block_grouping_test.rs` existing specs — the parallel-tool ordering fix.
- `openapi::emit_ts::tests::types_ts_parity` — must stay green untouched; if it reddens I changed
  an exposed type by accident (per PLAN_AUDIT "OpenAPI regen").
