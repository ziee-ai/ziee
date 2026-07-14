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

- **TEST-5** — reverted `results_by_id.clear()` in the worktree → panicked with
  *"X must carry its REAL result, not the stale pre-flush orphan"*. Restored. (Its
  scope was later corrected: the capture guard now subsumes `clear()`, so TEST-5
  covers the flushed half and TEST-16 the no-flush half — see FIX_ROUND-2.)
- **TEST-14** — written BEFORE fixing the cross-turn scoping bug and run against my own
  round-1 code → failed `assertion left: 3, right: 4` (turn 2's entire Tool message
  deleted). This is the regression that would have broken every gpt-oss conversation.

Honestly labelled as NON-discriminating (recorded rather than overclaimed):
- **TEST-10** passes with the claim reordering reverted (the pre-fix post-execution
  DELETE succeeded on the happy path). It is a regression guard; **TEST-13** pins the
  decision the fix turns on.
- **TEST-1** cannot fail pre-fix (it calls a fn that did not exist), so it carries an
  explicit CONTROL asserting the pre-fix blind-append really yields `["A","B","B"]`,
  with **TEST-17** (`#[should_panic]`) proving the invariant assertion is not vacuous.

## Not run (and why)

- E2E / `npm run check` / `gate:ui` — no frontend path in the diff.
- The **live end-to-end confirmation** khoi asked for (own stack on a free port, real
  Anthropic key read read-only from the `:8080` container's DB, mixed built-in +
  approval-required batch → approve → observe the 400 disappear) is **still outstanding**
  and tracked as the remaining work; it is not covered by any tier above.
