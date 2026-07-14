# FIX_ROUND-4 — fix-duplicate-tool-result

Round 4: a full blind re-audit of the round-3 diff. **Both production mechanisms clean
again**, every test's honesty verified ("I found no test whose assertions are narrower
than its claim"). Two findings, both in docs this branch added — the fourth consecutive
round to find doc drift, which is itself the signal worth naming: I keep writing
confident prose that outruns the code beneath it.

## Fixed

- **The model-facing copy lied.** The non-Won claim result told the model *"It was not
  executed twice"*, and the adjacent comment scoped the stale-error-wins cost to
  `AlreadyClaimed` (*"needs two concurrent passes over one branch"*). Both false on the
  `Failed` path: a failed DELETE leaves the row at `status='approved'`, so
  `after_llm_call`'s own `get_approved_tools_for_branch` re-claims and executes the tool
  **later in the same turn** — one pass, one flaky DELETE, identical cost. The copy now
  states only what is true on BOTH paths ("was not run by this request") and explicitly
  refuses to claim the tool never ran anywhere, because we cannot know. The comment
  names the `Failed` route to the same cost.
- **`append_content`'s header denied a caller that exists.** It claimed a retry *"would
  be dead code guarding a call shape that does not exist"*. The MCP extension's detached
  elicitation task calls `append_content_with_id` — same MAX+1-inside-INSERT slot
  computation, same `message_id` — concurrently with the approval loop's appends.
  Verified by reading both. The doc now records that gap honestly (narrow, pre-existing,
  a retry loop is out of scope) instead of asserting it away. This is exactly the defect
  ITEM-5/TEST-12 exist to catch, committed by the change that fixed ITEM-5.

## Verified clean (re-derived by the auditor, not taken on trust)

- Every one of the six loop exits pushes exactly one `tool_results` + one
  `executed_tool_use_ids` entry; **no `?` anywhere inside the loop body**, so no claim
  can be lost mid-batch.
- dedup's SCOPE doc matches the code; the reset is strictly finer-grained than
  `resolve_unique_tool_use_id`'s per-`message_id` guarantee, so it cannot false-drop; a
  Tool turn is always preceded by an Assistant-with-tool_use that clears `seen`, so it is
  never emptied/stranded. Ordering vs `clear_old_tool_results` verified.
- `replace_or_collect`'s `unwrap_or(messages.len())` yields an empty search slice (the
  `unwrap_or(0)` inversion is gone); the `_ =>` pass-through arm is genuinely
  unreachable, matching its "shouldn't happen" doc.
- The capture guard + `flush_assistant_tool_pair`'s "provably empty" argument re-derived
  airtight. TEST-5/TEST-5b confirmed to genuinely fail on base.
- Migration 158: every claim re-checked against the real migrations; 158 is the next free
  number; TEST-12's substring patterns match 124's actual formatting.

## Prose note, considered and kept (not a defect)

The auditor flagged that migration 158 + TEST-11/12 + the `append_content` doc rewrite
are a redundant-index cleanup **unrelated** to "never send two tool_result blocks", and
that this adds DDL deploy risk to a bugfix branch — "worth a conscious keep/split
decision". It is conscious: khoi explicitly chose "fix all 4 adjacent defects" from an
option picker after being shown these were NOT the cause. Called out in the PR so the
reviewer can split it if they disagree.

**New confirmed findings:** 2
