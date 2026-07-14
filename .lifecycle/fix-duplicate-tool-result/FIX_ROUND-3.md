# FIX_ROUND-3 — fix-duplicate-tool-result

Round 3: a full blind re-audit of the round-2 diff. **Both production mechanisms came
back sound** — the auditor independently re-derived and cleared them rather than take
them on trust. Every remaining finding was documentation drift, which matters here
because it is exactly the defect class ITEM-5/TEST-12 exist to catch: I had written
comments that argued against the code beneath them.

## Independently verified sound (not assumed — re-derived by a blind reviewer)

- **The claim loop.** All six exits traced (non-Won, `server_id: None`,
  server-not-found, sampling-no-session, connect-fail, normal): every one pushes
  exactly one `tool_results` entry + one `executed_tool_use_ids` entry, and there is no
  `?` inside the loop body. No path continues without pushing; no tool_use is left
  unpaired. The `Ok(false)` vs `Err` distinction is race-sound (autocommit single
  statement, READ COMMITTED row lock → the loser sees `rows_affected() == 0`).
- **The `debug_assert!` cannot fire.** Proof: a key enters `results_by_id` only when
  `pending_ids.remove(&K)` returned true; `K` only enters `pending_ids` via a
  `ToolUse{id: K}` that in the same arm pushes onto `current_tool_uses`; that vec is
  drained ONLY by the flush, whose loop removes every element's id. Keep-first also
  still holds (`insert` can never overwrite — the id left `pending_ids` on first
  capture). Noted: the capture guard is a strict improvement — the OLD `or_insert`
  admitted orphans, so `[use A, use B, result A, result Z, result B]` would have left
  `Z` behind and the new assert would have fired on the old code. Verified live: 319
  chat+mcp tests pass in a DEBUG build.
- **Slicing.** `messages[batch_start..]` with `batch_start == messages.len()` is an
  empty slice, not a panic.
- **Migration 158 + both DB tests.** The auditor ran the integration test's `pg_index`
  query against a real migrated build DB (`_sqlx_migrations` version 158 present) →
  returns exactly `uq_message_contents_message_sequence`, and confirmed
  `unnest(x.indkey)` works (int2vector is binary-coercible to `int2[]`) — the one thing
  in that query that could have silently errored.
- **Security / concurrency / perf.** Clean; nothing touches auth/secrets/SSRF, logs
  emit only tool_use_ids, dedup is negligible next to a provider call.

## REFUTED by running it — the reordering IS discriminated

The auditor's strongest remaining claim was that *"the branch's riskiest change ships
with zero discriminating coverage"* — that no test fails with the claim reordering
reverted. I tested the claim instead of arguing with it (B7): a revert-probe that
disables the claim at the loop head and restores a post-execution delete makes

    mcp::mcp_approval_loop_test::mcp_approval_loop_unresolvable_tool_errors_and_terminates ... FAILED

That arm `continue`s BEFORE any execution, and FIX_ROUND-1 removed its private delete,
so its row can only disappear if the claim precedes execution. Reverting the ordering
⇒ the row survives ⇒ the loop spins to max_iteration ⇒ the test fails. Probe reverted;
tree verified clean.

The auditor could not have known this — it requires knowing that the arm's own delete
was removed in an earlier round, which is invisible in a diff-only view. Recorded as
`rejected` in the ledger with the evidence, and TEST_RESULTS.md now cites the probe.

## Fixed — documentation that argued against its own code

- **`ClaimOutcome::AlreadyClaimed`'s doc** said *"Skip WITHOUT emitting anything — an
  error result here would be the second answer"*. That was round-1's disposition, which
  round 2 proved bricks the branch; the doc survived the fix. It now argued FOR the
  hazard the code closes. Rewritten.
- **`ClaimOutcome::Failed`'s doc** said *"fail the turn loudly"* — no path does. Rewritten.
- **`dedup_tool_results_by_id`'s summary** (and the caller's `warn!`) claimed it
  enforces one result per id *"across the whole request"*, which its own SCOPE paragraph
  three lines below correctly narrows to one turn group. Headline corrected; the
  overclaim is the exact thing that made the round-1 global-scope bug feel right.
- **The "deduped keep-first at assembly" comment** was wrong in a way worth stating
  plainly: on `AlreadyClaimed` this pass persists its error result while the winner is
  still executing, so the ERROR takes the lower `sequence_order` and keep-first makes it
  authoritative — the model reads "not run here" and never sees the winner's output.
  The comment now names that cost and why it is still the right trade (the alternative
  is not a worse answer but a dead conversation), instead of implying the real result
  survives.
- **TEST-18's doc** implied it guards this diff; it passes on base. Relabelled a
  CHARACTERIZATION test — it pins pre-existing behavior *because that behavior is the
  load-bearing premise* for the claim-path decision, so a future edit that synthesized
  a result there would silently invalidate it.

## Known limit, recorded rather than papered over

The auditor constructed a shape the batch-scoped dedup does NOT catch:
`[Assistant{use A}, Tool{result A}, Assistant{use B}, Tool{result B}, User{result A}]`
— `seen.clear()` at `Assistant{use B}` forgets `A`, so a trailing duplicate `result A`
survives. They could not confirm an end-to-end reachable sequence and filed only the
doc half, and I could not either: it needs a stale branch-scoped `approved`/`denied` row
from an OLDER assistant message, and `after_llm_call` tends to consume such a row first
while the new capture guard drops its orphan result. Left as-is deliberately: the
per-turn-group scoping is REQUIRED for the gpt-oss constant-id case (TEST-14), so the
correct fix would narrow the INPUT (the branch-scoped approval lookup), not the dedup —
a different defect, out of this feature's scope. Noted here so the next person finds the
analysis rather than re-deriving it.

**New confirmed findings:** 5
