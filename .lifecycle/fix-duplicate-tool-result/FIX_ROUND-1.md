# FIX_ROUND-1 — fix-duplicate-tool-result

Round 1 of the fix/re-audit loop. The blind audit (4 agents, 14 angles, diff-only
context) found **three HIGH defects I introduced**. Two of them are regressions of the
exact class this feature exists to prevent — three agents converged on the worst one
independently, which is why it counts.

## HIGH-1 + HIGH-2 — the same root error: `tool_use_id` is NOT globally unique

`resolve_unique_tool_use_id` (`mcp.rs:326`) seeds its used-set from
`SELECT ... WHERE message_id = $1` — uniqueness is scoped to **one assistant
message** — and its own doc names the case it tolerates: gpt-oss/harmony streams the
non-unique constant `"tool_use"` for every call. A later turn is a NEW message, so the
same id legitimately recurs, answering a DIFFERENT tool_use. OpenAI-compatible
providers pair `tool_call_id` per adjacent turn, not globally.

Both my new functions keyed across the **whole request**:
- `dedup_tool_results_by_id` dropped the later turn's real result → its tool_use
  unpaired → **the sibling `chat-toolresult-pairing` 400 I promised not to
  reintroduce**, for every gpt-oss conversation.
- `replace_or_collect_tool_results` overwrote an OLDER turn's result (corrupting
  history) and reported no leftover → the CURRENT tool_use unanswered.

**Proved before fixing** (B7 — verification means running it): wrote
`dedup_tool_results_allows_the_same_id_reused_across_turns` and watched it FAIL on my
own code (`assertion left: 3, right: 4` — turn 2's entire Tool message deleted).

**Fixed** by scoping both to one turn group: dedup resets its seen-set at each
Assistant message that opens a tool batch; `replace_or_collect` searches only from the
last Assistant-with-tool_use (`rposition`) onward. New regression tests:
`dedup_tool_results_allows_the_same_id_reused_across_turns`,
`replace_or_collect_ignores_the_same_id_in_an_older_turn`.

Why the original tests missed it: every fixture used globally-distinct ids
(`A`/`B`/`i1`/`i2a`). The audit's value was seeing the fixture bias, not the code.

## HIGH-3 — the claim wasn't a claim

`delete_tool_approval` returns `Ok(rows_affected() > 0)` (`approval/repository.rs:390`).
My claim matched only on `Err`, discarding the bool — so `Ok(false)` (the row already
claimed by a concurrent pass: *precisely the race the claim exists to close*) fell
through and executed. The comment asserted exactly-once; the code didn't implement it.

Compounding it (MEDIUM), the claim-FAILURE arm pushed an `is_error` result and
continued while **leaving the row intact** — so `after_llm_call` re-fetched it,
executed for real, and persisted a SECOND `tool_result`. My "skip" produced the
duplicate it was written to prevent.

**Fixed** by making the verdict explicit and total — extracted `claim_outcome()` +
`ClaimOutcome{Won, AlreadyClaimed, Failed}`:
- `Ok(true)` → Won → execute.
- `Ok(false)` → AlreadyClaimed → skip **silently** (the winner produces the result;
  emitting anything here would be the second answer).
- `Err` → Failed → **propagate** (`return Err(e)`). We cannot know whether the row
  survives, so we can neither safely execute nor safely skip. Fail the turn loudly.

This **revises DEC-4**, whose "skip with an is_error result" disposition the audit
falsified. Recorded in DECISIONS.md as DEC-4 (revised) + DEC-10.

The extraction also produced the discriminating test the integration test could not be
(`claim_outcome_distinguishes_won_already_claimed_and_failed`), answering the
test-reality HIGH: `approved_tool_is_claimed_and_executes_exactly_once` passes with
the fix reverted, because the bug needs a losing/failing DELETE the HTTP harness cannot
induce. It stays as an exactly-once guard (honest scope already recorded in
TESTS.md/DRIFT-1.5); the unit test is what actually discriminates.

## MEDIUM — `results_by_id.clear()` only half-closed the orphan hazard

`[result X(stale), use X, result X(real)]` never flushes before X's real result, so
`clear()` cannot help and keep-first still emitted the stale orphan. TEST-5 only
covered the flushed half.

**Fixed at the capture rule** instead of the eviction: a result is captured only if it
answers a tool_use still OUTSTANDING in this batch (`pending_ids.remove(&id)` is both
the resolve and the test). Orphans are dropped on arrival and can never shadow anything.
`clear()` stays as belt-and-braces. New test:
`group_assistant_blocks_orphan_before_its_use_does_not_shadow_the_real_result`.

## MEDIUM/LOW — test quality (all fixed)

- **Global panic hook**: my `catch_unwind` control swapped the PROCESS-GLOBAL panic
  hook — under `cargo test`'s parallel threads it would swallow a genuinely failing
  test's output. My "fix" for stderr noise was worse than the noise. Replaced with a
  `#[should_panic]` test (`invariant_assertion_catches_a_duplicate`); the control now
  just asserts the pre-fix ids are `["A","B","B"]`, which is the real proof anyway.
- **Prose-linting**: the doc test matched the arbitrary literal `"is the next step"`
  (any paraphrase passes), and the wsl2 precedent it cited scans for a CODE pattern,
  not prose. Rewritten to check the doc against the **schema**: the constraint the
  comment cites must actually be created by a migration — so a rename or removal fails.
- **Shallow no-op assert**: compared only role + `content.len()`. Now deep-compares via
  `serde_json::to_value`.
- **Hidden tradeoff**: `resume_shape_keeps_exactly_one_result_per_tool_use` asserted
  only shape, silently certifying that the defense keeps the PLACEHOLDER and drops the
  real result. Now asserts that explicitly, with a comment explaining it is deliberate
  (the defense buys validity; the source fix buys fidelity) and pointing at DEC-1/DEC-2
  if it ever flips.
- **Any-error assert**: `assert!(collide.is_err())` → asserts `db.is_unique_violation()`.
- **Stale doc**: `delete_tool_approval`'s "(after execution)" header rewritten to
  document the claim contract and that callers must not discard the bool.
- **Uncorrelated warn**: made `dedup_tool_results_by_id` return the dropped ids instead
  of logging inside; the call site logs them with `conversation_id`/`message_id`/
  `iteration`. Purer fn, diagnosable warn — and the tests now assert the returned ids,
  which strengthened them.
- **Moot**: the i18n/copy and `executed_tool_use_ids` findings both targeted the
  claim-failure arm's fabricated result, which no longer exists (Err now propagates).

## Rejected (with rationale — a dismissal is not a fix)

- **Duplicated test helpers** (`approval_claim_test.rs` vs `mcp_approval_loop_test.rs`):
  rejected. Per-file fixture duplication is this suite's established idiom —
  `assistant_block_grouping_test.rs` duplicates `streaming.rs`'s in-module fixtures for
  the same reason. Promoting them would edit shared test infrastructure to suit one
  feature, which B3 explicitly forbids.
- **dedup HashSet clone cost**: rejected. O(total blocks) with a short-String clone per
  result, immediately followed by `clear_old_tool_results` walking the same data at
  O(total *chars*) — dedup is strictly cheaper than its own neighbor, and both are noise
  against the network LLM call. (The perf auditor independently reached the same
  conclusion and cleared it.)
- **Migration 158 / api-contract**: rejected — verified safe and regen-free by two
  agents independently (a unique index is not an access-control mechanism; `DROP INDEX`
  cannot remove a constraint's backing index, so a name confusion fails loudly).

> **Correction.** This round originally ended `New confirmed findings: 0`, written
> BEFORE the re-audit had been run — i.e. I self-certified, which is exactly what the
> gate forbids (P1). The round-2 blind re-audit then found **7**, two of them HIGH and
> in this very round's ITEM-4 fix. The count below is the honest one. Recorded rather
> than quietly edited, because the mistake is the point: a fix round is not done when
> the fixes are written, only when a fresh blind round says so.

**New confirmed findings:** 7
