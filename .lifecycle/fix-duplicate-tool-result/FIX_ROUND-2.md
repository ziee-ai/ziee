# FIX_ROUND-2 — fix-duplicate-tool-result

Round 2: a full blind re-audit of the round-1 diff (fresh agents, diff-only context).
It found **two more HIGH defects, both in my round-1 fix for ITEM-4** — one of which
made the failure mode *worse than the bug this feature exists to remove*. Round 1's
`New confirmed findings: 0` was therefore premature; corrected to 7 below.

## HIGH-4 — claim-then-execute traded a recoverable bug for an unrecoverable one

DEC-4/DEC-10 rested on a mitigation I asserted but never RAN (B7): *"a crash between
the claim and the result append leaves the tool un-run; its tool_use then gets a
synthesized `is_error` placeholder on the next turn — degraded but valid."*

**That is false**, and the audit traced why. `group_assistant_blocks`' trailing branch
gates synthesis on `batch_has_result` (`streaming.rs:1814`): when NOTHING in the batch
produced a result — the single-tool approval, i.e. the common shape — it emits a
**bare Assistant turn with no Tool message at all**. That is correct ONLY for the
awaiting-approval case, whose result is still coming. For a tool that will never
produce one, the `tool_use` is unpaired on EVERY subsequent request in that branch:
the provider rejects all of them and **the conversation is bricked** — the exact
sibling failure (`chat-toolresult-pairing`) this must not reintroduce.

So my round-1 code turned "an expensive tool might run twice" (recoverable; duplicate
ROWS are deduped keep-first at assembly) into "the branch is permanently unusable".
A strictly worse trade.

**Proved by running it**, not by reading: added
`group_assistant_blocks_resultless_batch_emits_a_bare_unpaired_assistant_turn`
(TEST-18), which pins the bare-unpaired-turn behavior so the hazard is a test, not a
comment.

## HIGH-5 — `return Err` discarded the batch's already-executed results

Both call sites propagate with `?`, so a transient DB error on approval *B*'s claim
threw away approval *A*'s real result — after A had already won its claim, executed,
and had its row deleted. A never re-runs, its result is never persisted, and its
tool_use joins B's in the unpaired graveyard. Pre-fix this error was swallowed and A's
result survived.

## The fix for both: every claim path emits exactly one result, and never bails

`AlreadyClaimed` and `Failed` now push an `is_error` result + `executed_tool_use_ids`
and `continue` — matching what every sibling error arm in this loop already does, and
for the reason `mcp.rs:1207` documents verbatim. The invariant is now explicit: **every
approval in the batch yields exactly one result**, so no path can abandon a tool_use.

Exactly-once execution is preserved where it matters: `Ok(true)` remains the ONLY
branch that runs the tool. In the rare `Failed` case the row may survive and a later
pass may re-execute → two result ROWS → deduped keep-first at assembly → a VALID
request. That is the correct priority: validity is unrecoverable, a duplicate is not.

This supersedes DEC-10's `Err → propagate` disposition (recorded there).

## MEDIUM/LOW — also fixed

- **`batch_start … .unwrap_or(0)`** inverted `replace_or_collect_tool_results`' own
  documented SCOPE rule when no Assistant-with-tool_use exists: it searched the WHOLE
  request, re-opening the cross-turn overwrite. Now `unwrap_or(messages.len())` —
  fail-safe "search nothing".
- **`results_by_id.clear()` was dead code.** The round-1 capture guard makes the map's
  keys a subset of `current_tool_uses`' ids, all drained by the flush loop's `remove`.
  Replaced with a `debug_assert!` documenting the invariant, so the claim is checked
  rather than implied by a no-op.
- **TEST-5's doc overclaimed**: its fixture is handled by the capture guard alone, so
  it passes with `clear()` deleted and did not discriminate what it named. Doc
  corrected to say what it actually covers (the flushed half); TEST-16 covers the
  harder no-flush half.
- **TEST-10's file header overclaimed** ("drives the REAL path", "the decisive
  assertions") for a test that passes pre-fix. Header now states its honest scope: a
  regression guard for the reordering + the removal of four scattered deletes;
  TEST-13 (`claim_outcome`) is the leg that discriminates.

## Re-verified clean this round (recorded so the coverage is real, not assumed)

The auditor independently re-derived and CLEARED the round-1 fixes rather than take
them on trust:
- **dedup turn-group scoping** — ran the counterexamples (text-only Assistant;
  tool_results before any Assistant; gpt-oss cross-turn reuse; the
  `Assistant{A,B}/Tool{rA,rB}/User{rB dup}` bug shape). The reset lands before the Tool
  turn carrying the scope's results, which is exactly right. `emptied`/`retain` index
  arithmetic sound (`Vec::retain` visits each element once, in order).
- **keep-first after the capture-guard change** — `use A, result A, result A-dup` with
  and without an intervening flush: the dup is still dropped, `batch_has_result` still
  reads correctly.
- **No leak on the (now removed) `return Err`** — the spawned elicitation task is
  anchored to `elicit_notify_rx` and ends when the tx drops at function exit;
  `_guard`/`_owned` are per-iteration bindings declared after the claim.
- **Security / migration 158** — no auth surface touched; logs emit only tool_use_ids;
  `DROP INDEX` provably cannot remove the constraint's backing index. Also noted: 114's
  index served `WHERE message_id = ? ORDER BY sequence_order`, but 124's constraint
  index has the same leading column, so there is no plan regression.

**New confirmed findings:** 7
