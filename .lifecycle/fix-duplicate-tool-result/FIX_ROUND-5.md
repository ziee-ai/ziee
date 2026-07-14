# FIX_ROUND-5 — fix-duplicate-tool-result

Round 5: a full blind re-audit of the round-4 diff. Three findings — one procedural
(caught by the auditor, not by me), two more stale comments.

## The one that mattered most: probe edits left in the working tree

The auditor opened with *"the stated premise is false: the tree is not clean"* and found
my temporary `LIVE-PROBE` edits — which **disabled BOTH halves of the fix** (dedup
replaced by an empty `Vec`, `replace_or_collect` reverted to blind append) — sitting
uncommitted while I ran the live pre-fix repro. HEAD was correct and that is what it
reviewed, but as it noted: *"a stray `git commit -a` would ship the fix disabled"*, and
anything built from that tree runs pre-fix code.

Exactly right, and worth recording rather than waving off as "intentional, I knew":
the danger of a revert-probe is not the probe, it is forgetting it. Restored from
backup; `git diff -- src-app/server/src` verified EMPTY against HEAD before proceeding.
This is what A2 (clean working tree) exists for, caught by a reviewer rather than the
gate, because the gate only runs at phase 8.

## Two more stale comments (fixed)

- **`append_content`'s header misattributed the swallow.** Round 4's own fix said "the
  approval-loop site swallows the error with `let _ =`". It does not — `mcp.rs`'s
  approval-loop appends use `match` / `if let Err(e)` + `tracing::error!`. The `let _ =`
  belongs to the OTHER racer the same sentence had just named: the elicitation task's
  `append_content_with_id`. So the substantive claim (a lost race drops a row; neither
  retries) was right, but it pointed the reader at the wrong call site and its "rather
  than a loud failure" clause was false for the site named. Corrected.
- **A stale survivor in `group_assistant_blocks`' pure-text branch**: *"Any leftover
  results in `results_by_id` are orphans with no tool_use — dropped here."* Round 2 moved
  orphan-dropping to CAPTURE time, so that state is unreachable — the diff updated
  `flush_assistant_tool_pair`'s header for the new model but missed this caller-side
  comment describing the old one. Corrected.

## Verified clean

- All six approval-loop exits push exactly one `tool_results` entry; no `?` in the loop
  body — so the comment's claim that a `return Err` would strand earlier winners
  describes a hazard the code genuinely avoids.
- Scoping (both fns), the capture guard, `flush_assistant_tool_pair`'s "provably empty"
  argument, and the `Failed`-claim "known cost" paragraph all re-verified accurate — the
  auditor confirmed two `tool_result` rows do land and keep-first collapses them to one
  on the wire.
- **test-reality: no undisclosed free-riders.** Every test revert-traced. TEST-5/5b
  genuinely fail pre-fix; TEST-18, `approved_tool_is_claimed_and_executes_exactly_once`
  and TEST-13 all pass on base AND all say so in their own docs; migration 158's
  integration test fails without 158.

## Coverage gap it raised, and my answer

The auditor noted `approval_claim_test` builds a `StubChat` exposing `requests()` /
`last_request()` (verbatim captured wire bodies) yet never asserts the outbound request
is free of duplicate `tool_result` blocks — "the feature's headline behavior has no
integration-level assertion". It filed this as a gap, not a defect, because reproducing
the mixed batch through that harness is real setup.

Answered, and better than a stub could: the headline behavior is now proven **against
real Anthropic** (see TEST_RESULTS.md's live A/B). The pre-fix binary produced the
reported error verbatim — `messages.2.content.2: each tool_use must have a single
result. Found multiple tool_result blocks with id: toolu_01RHUaXDdW…` — and the fixed
binary completed the identical flow with both results. A stub asserting the wire body
would be a weaker version of that.

**New confirmed findings:** 3
