# TEST_RESULTS — fix-duplicate-tool-result

Backend-only diff (BASE.md): no frontend workspace is touched, so the `npm run check` /
`gate:ui` / e2e chain does not apply. No new permission ⇒ A9/A10 do not apply.

Full logs (P4 — captured, not tailed):
- unit: `/data/khoi/home-workspace/ziee/tmp/lifecycle-logs/dup-toolresult-unit.log`
- integration: `/data/khoi/home-workspace/ziee/tmp/lifecycle-logs/dup-toolresult-int.log`

```bash
# unit — 51 passed, 0 failed
cargo test --lib -p ziee -- chat::core::services::streaming:: \
    chat::core::repository::contents:: mcp::chat_extension::mcp::tests

# integration — 12 passed, 0 failed
cargo test --test integration_tests -- --test-threads=4 \
    chat::assistant_block_grouping chat::append_content_ordering \
    mcp::approval_claim mcp::mcp_approval_loop
```

## Per-TEST results

- **TEST-1**: PASS — `mcp::chat_extension::mcp::tests::resume_of_a_mixed_batch_yields_exactly_one_result_per_tool_use`
- **TEST-2**: PASS — `streaming::tests::dedup_tool_results_keeps_first_and_drops_later_duplicates`
- **TEST-3**: PASS — `streaming::tests::dedup_tool_results_is_a_noop_on_a_valid_request`
- **TEST-4**: PASS — `streaming::tests::dedup_tool_results_removes_a_message_it_empties`
- **TEST-5**: PASS — `streaming::tests::group_assistant_blocks_later_real_result_beats_stale_orphan`
- **TEST-6**: PASS — `mcp::chat_extension::mcp::tests::replace_or_collect_replaces_an_existing_result_in_place`
- **TEST-7**: PASS — `mcp::chat_extension::mcp::tests::replace_or_collect_returns_a_result_with_no_existing_block`
- **TEST-8**: PASS — `mcp::chat_extension::mcp::tests::replace_or_collect_handles_a_mixed_batch`
- **TEST-9**: PASS — `chat::assistant_block_grouping_test::resume_shape_keeps_exactly_one_result_per_tool_use` (+ `dedup_leaves_a_valid_multi_iteration_request_untouched`)
- **TEST-10**: PASS — `mcp::approval_claim_test::approved_tool_is_claimed_and_executes_exactly_once`
- **TEST-11**: PASS — `chat::append_content_ordering_test::message_contents_has_exactly_one_unique_sequence_guard`
- **TEST-12**: PASS — `chat::core::repository::contents::tests::append_content_doc_cites_a_constraint_that_really_exists`
- **TEST-13**: PASS — `mcp::chat_extension::mcp::tests::claim_outcome_distinguishes_won_already_claimed_and_failed`
- **TEST-14**: PASS — `streaming::tests::dedup_tool_results_allows_the_same_id_reused_across_turns`
- **TEST-15**: PASS — `mcp::chat_extension::mcp::tests::replace_or_collect_ignores_the_same_id_in_an_older_turn`
- **TEST-16**: PASS — `streaming::tests::group_assistant_blocks_orphan_before_its_use_does_not_shadow_the_real_result`
- **TEST-17**: PASS — `mcp::chat_extension::mcp::tests::invariant_assertion_catches_a_duplicate` (`#[should_panic]`)
- **TEST-18**: PASS — `streaming::tests::group_assistant_blocks_resultless_batch_emits_a_bare_unpaired_assistant_turn`

## Regression guards (pre-existing, must stay green)

- **PASS** — the sibling `chat-toolresult-pairing` suite: `group_assistant_blocks_*`
  (7 unit) + `chat::assistant_block_grouping_test` (5 integration: parallel-per-iteration,
  corrupted-interleaving, trailing-tool_use, partial-parallel-synthesis,
  multi-iteration). My ITEM-2/3 changes sit directly on this code.
- **PASS** — `mcp::mcp_approval_loop_test::{mcp_approval_loop_bare_name_recovers_and_executes,
  mcp_approval_loop_unresolvable_tool_errors_and_terminates}`. The second is a free
  guard for FIX_ROUND-1's removal of three redundant `delete_tool_approval` calls: that
  arm returns BEFORE any execution, so its row can only be gone if the claim precedes
  execution — if the claim regressed, it spins to max_iteration and fails.
- **PASS** — `mod trim_tests` (7 unit): `clear_old_tool_results` is exonerated and
  untouched; these prove the dedup running immediately before it did not disturb the
  keep-last-K window.
- **PASS** — `chat::append_content_ordering_test::append_content_yields_monotonic_sequence_order_for_parallel_tool_iteration`
  — the atomic `MAX+1` parallel-tool ordering fix is not regressed.
- **PASS** — full `cargo test --lib -p ziee -- chat:: mcp::`: **319 passed, 0 failed**,
  in a DEBUG build, so the new `debug_assert!(results_by_id.is_empty())` in
  `flush_assistant_tool_pair` is live across every case the suite reaches and never fires.

## Verified-to-fail-without-the-fix (the tests that actually discriminate)

Claiming a test "fails pre-fix" is only honest if it was RUN that way (B7). Two were:

- `TEST-5` — reverted `results_by_id.clear()` in the worktree → panicked with
  *"X must carry its REAL result, not the stale pre-flush orphan"*. Restored. (Its
  scope was later corrected: the capture guard now subsumes `clear()`, so TEST-5
  covers the flushed half and TEST-16 the no-flush half — see FIX_ROUND-2.)
- `TEST-14` — written BEFORE fixing the cross-turn scoping bug and run against my own
  round-1 code → failed `assertion left: 3, right: 4` (turn 2's entire Tool message
  deleted). This is the regression that would have broken every gpt-oss conversation.

- The claim REORDERING — a blind auditor asserted "the branch's riskiest change ships
  with zero discriminating coverage". Tested rather than argued: a revert-probe (claim
  disabled at the loop head, delete restored after execution) makes
  `mcp_approval_loop_unresolvable_tool_errors_and_terminates` **FAIL**. That arm
  `continue`s BEFORE execution and this branch removed its private delete, so its row can
  only disappear if the claim precedes execution; revert the ordering ⇒ the row survives
  ⇒ the loop spins to max_iteration ⇒ red. The claim IS discriminated end-to-end. Probe
  reverted; tree verified clean.

Honestly labelled as NON-discriminating (recorded rather than overclaimed):
- `TEST-10` passes with the claim reordering reverted (the pre-fix post-execution
  DELETE succeeded on the happy path). It is a regression guard; **TEST-13** pins the
  decision the fix turns on, and the revert-probe above covers the ordering.
- `TEST-1` cannot fail pre-fix (it calls a fn that did not exist), so it carries an
  explicit CONTROL asserting the pre-fix blind-append really yields `["A","B","B"]`,
  with **TEST-17** (`#[should_panic]`) proving the invariant assertion is not vacuous.

## LIVE A/B CONFIRMATION — the reported bug reproduced and fixed, end-to-end

Run on my OWN stack (`:8231`, isolated embedded Postgres, scratch data dirs) with the
real Anthropic key khoi supplied. **`:8080` was never written to, never restarted** —
40h uptime unbroken; only read-only `docker exec psql` provider listing touched it.
Stack torn down after the run.

**Forcing function** (a mixed batch — the precondition the root-cause analysis
predicted): a KB attached so the approval-EXEMPT built-in `knowledge_base` attaches,
plus the approval-REQUIRED external `fetch` server, `approval_mode=manual_approve`,
and a prompt telling real Claude to emit both `tool_use` blocks in ONE parallel batch.

Claude complied, and the DB reached exactly the predicted state:

```
0 tool_use    list_knowledge_bases   ← built-in, approval-exempt: ran
1 tool_use    fetch                  ← external: paused (mcpApprovalRequired)
2 tool_result list_knowledge_bases   ← ONLY the built-in's result persisted
```

Then approve → resume. Identical flow, identical prompt, one variable (the fix):

| Binary | Result |
|---|---|
| **PRE-FIX** (both call sites reverted, rebuilt) | `AI provider error: Invalid request: messages.2.content.2: each tool_use must have a single result. Found multiple `tool_result` blocks with id: toolu_01RHUaXDdW…` |
| **POST-FIX** (HEAD, rebuilt) | `complete` / `finish_reason: stop` — blocks `[tool_use, tool_use, tool_result, tool_result, text]`, Claude answered *"Both calls completed successfully"* |

That pre-fix line is the USER'S REPORTED SYMPTOM verbatim (theirs was
`messages.14.content.2`, same error, deeper history). The mechanism is confirmed live,
not just by code trace.

**The defense never fired:** `grep -c "dropping a duplicate tool_result"` = **0** on the
post-fix run. The SOURCE fix (`replace_or_collect_tool_results`) handled it in place, so
`dedup_tool_results_by_id` had nothing to drop — exactly the designed division of labour
(ITEM-1 fixes the cause; ITEM-2 is the tripwire that should stay silent).

Two incidental pre-existing issues surfaced while standing the instance up, neither
related to this fix and both out of scope (noted for the PR): a provider registered
without a `/v1` base_url yields an empty `AI provider error: Invalid request:` with no
detail, and `claude-sonnet-4-5` resolves to *adaptive* thinking, which Anthropic rejects
("adaptive thinking is not supported on this model") until `supports_thinking:false` is
set on the model row.

## Not run (and why)

- E2E / `npm run check` / `gate:ui` — no frontend path in the diff.
