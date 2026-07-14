# FIX_ROUND-7 — fix-duplicate-tool-result

Round 7: a full blind re-audit of the round-6 diff. **One** finding, LOW and doc-only —
and a good one: a doc that contradicted *itself*.

## Fixed

`append_content`'s header opened with **"Assumes appends to a single `message_id` are
sequential — the streaming agentic loop awaits each append in one task, which is the
only production caller"** … and eight lines below, the paragraph FIX_ROUND-4 added to
correct this very doc said **"A concurrent caller DOES exist, so this is not
hypothetical"**. The doc asserted both that appends are sequential and that they are not.

I fixed the false claim in round 4 by *appending* a correction instead of *reconciling*
the text, which is how a doc ends up arguing with itself. The header now says one thing:
appends are mostly-but-not-strictly sequential, names the second writer (the detached
elicitation task's `append_content_with_id`), and keeps the honest gap note (nothing
retries; the elicitation side swallows a lost race and drops the row; a retry loop is out
of scope).

Worth noting where this landed: on the very function whose stale doc was ITEM-5, guarded
by TEST-12 — which checks the doc names a REAL constraint, not that the doc is
self-consistent. A test can pin a citation; it can't pin coherence. Four rounds of doc
drift on this branch is the honest signal that my prose runs ahead of my code.

## Verified sound (re-derived independently, not asserted)

- **Capture-guard invariant proved inductively**: `pending_ids` only gains ids
  simultaneously pushed to `current_tool_uses`; capture requires
  `pending_ids.remove(&id) == true`; both drain together in flush. So
  `keys(results_by_id) ⊆ ids(current_tool_uses)` always — the `debug_assert!` and both
  "provably empty" comments hold. Checked against duplicate-id and interleaved-batch
  shapes (`[use A, use A, result A]`, `[use A, use B, result A, use C, result B]`).
- **All SEVEN exits** of `execute_approved_tools_sync` push exactly one `tool_results` +
  one `executed_tool_use_ids` entry (the `is_final` arm pushes then returns — no
  double-push). The `expect("non-Won, non-AlreadyClaimed is Err")` is
  unreachable-by-construction.
- **The "Failed does NOT need a concurrent request" claim checks out**: `after_llm_call`
  STEP 1 (`mcp.rs:2271`) calls `get_approved_tools_for_branch` unconditionally, BEFORE
  the `executed_tool_use_ids` filter at `:2391`.
- **Both `delete_tool_approval` docs accurate**; only two callers exist (the claim + the
  denial cleanup), matching the narrowed directive.
- **Dedup scoping, `replace_or_collect`'s empty-slice default, migration 158 +
  TEST-11/12's SQL** — all re-verified; `unnest(indkey)` on `int2vector` is the standard
  idiom and the `pg_index` query correctly excludes the `(id)` PK.
- **Test reality**: every test that passes on base admits it in its own doc; the two
  orphan-shadow tests genuinely fail pre-fix (the old `or_insert` emits `"stale"` where
  they assert `"real"`).

## Noted, not filed (the auditor's own framing — recorded so it isn't lost)

Claim-before-execute **widens the crash window**: a process death between the DELETE and
result persistence leaves the row gone and the `tool_use` permanently unpaired (exactly
what TEST-18 documents), whereas the pre-fix ordering left the row at
`status='approved'` for a re-approve to recover. The auditor could not concretely justify
that the UI would re-send the approval to make the old path actually recover, so filed it
as the inherent at-most-once vs at-least-once trade this diff deliberately takes — the
one khoi chose in DEC-4, revised in DEC-10, and pinned by TEST-18. Not a defect, but the
sharpest statement yet of what ITEM-4 costs; flagged in the PR.

**New confirmed findings:** 1
