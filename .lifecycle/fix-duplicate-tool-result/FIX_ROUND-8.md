# FIX_ROUND-8 — fix-duplicate-tool-result

Round 8: a full blind re-audit of the round-7 diff. **Zero correctness, concurrency,
security, or api-contract defects** — the third consecutive round with none. Two LOW
doc/label findings, one of which round 7's own fix created.

## Fixed

- **`append_content_with_id`'s doc pointed at a retracted assumption.** It read "Same
  sequential-callers assumption — see `append_content`" — but round 7 rewrote
  `append_content`'s header to state the OPPOSITE (appends are not strictly sequential)
  and to name `append_content_with_id`'s own caller, the detached elicitation task, as
  the concurrent writer that breaks it. So the sibling doc cited an assumption the
  referenced doc explicitly retracts, and did so while being the very function that
  violates it. Rewritten to say what it actually is: the OTHER writer in that race, the
  one whose `let _ =` caller silently drops the losing row.

  This is the fifth round to find doc drift, and the third where **my own fix created
  the next round's finding** — round 4 fixed a false claim by appending a contradiction,
  round 7 fixed the contradiction and orphaned a cross-reference. Correcting a doc means
  re-reading everything that points AT it, not just the sentence in front of me.

- **A dangling test cross-reference.** The no-intervening-flush orphan test was labelled
  `TEST-5b` in source while its sibling's doc (and TESTS.md) call it `TEST-16`. Relabelled
  to `TEST-16` so the source, the cross-reference, and the enumeration agree.

## Verified sound (independently re-derived)

- **`append_content`'s header is now internally consistent AND matches the code**: the
  elicitation task really is a second `append_content_with_id` writer to the same
  `message_id` with the same MAX+1-inside-INSERT; `uq_message_contents_message_sequence`
  really is created by 124 and survives 158; the elicitation side really does `let _ = …`
  while the approval-loop sites `tracing::error!`.
- **Every approval path pushes exactly one `tool_results` entry** — the non-Won arm,
  no-server_id, server-not-found, sampling-no-session, connect-fail, the `is_final` early
  return, and the normal tail. "Removing the four scattered deletes left no path that
  skips the claim, and none double-pushes."
- **The capture guard genuinely discriminates BOTH halves**: hand-simulated, pre-fix
  `or_insert` emits `"stale"` for TEST-5 *and* TEST-16; post-fix both emit `"real"`.
- **`flush_assistant_tool_pair`'s "provably empty"** argument holds; the `debug_assert!`,
  the removed-`clear()` rationale, and the pure-text branch's claim are all sound.
- **Scoping**: dedup's seen-set reset and `replace_or_collect`'s
  `rposition(...).unwrap_or(messages.len())` agree on "current batch = last Assistant
  carrying a tool_use", and the `unwrap_or(len)` searches nothing rather than inverting
  the rule. Dedup-before-trim is correct — `clear_old_tool_results` counts `tool_result`
  positions and cannot itself remove messages.
- **Gemini pairing is safe**: `to_content_block` carries `name: Some(tool_name)` on the
  fresh result, so replacing a placeholder in place preserves name-based
  `functionResponse` pairing (the tests' `name: None` is fixture-only).
- **Test reality**: every test that passes with the fix reverted admits it in its own doc
  (TEST-13, TEST-18's characterization label, `approval_claim_test`'s HONEST SCOPE).
  No undisclosed free-riders.
- **Migration 158**: its quotes of 114/124 are accurate, no number collision, and
  TEST-11's `pg_index` query + unique-violation assertion match the suite's idiom.

## Explicitly considered and rejected by the auditor (recorded so it isn't re-litigated)

- The tension between `ClaimOutcome::Failed`'s "we cannot tell whether the row survives"
  and the call site's "a failed DELETE leaves the row at `status='approved'`" — judged a
  hedging nuance, not a falsehood: the latter establishes the reachable cost case, and
  `after_llm_call` does in fact re-call `get_approved_tools_for_branch`.
- The claim-before-execute at-most-once tradeoff (a crash between claim and persist could
  strand a batch) — not filed, "since the docs state that choice deliberately".

**New confirmed findings:** 2
