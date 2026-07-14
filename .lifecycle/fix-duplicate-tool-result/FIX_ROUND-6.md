# FIX_ROUND-6 — fix-duplicate-tool-result

Round 6: a full blind re-audit of the round-5 diff. **No correctness, concurrency,
security, or api-contract defect.** Two findings, both LOW and doc-only — and both the
same shape as rounds 3/4/5: a doc that outran its code.

## Fixed

- **The wrapper doc the call site actually sees.** Round 1 fixed
  `approval::repository::delete_tool_approval`'s "(after execution)" header — but the
  claim invokes it through `McpChatRepository::delete_tool_approval`
  (`Repos.chat.mcp`, `mcp.rs:814`), a **wrapper one hop from the call site** whose own
  doc still said "(after execution)". I fixed the doc a reader would reach second and
  missed the one they reach first. Both now describe the claim contract.
- **An absolute directive that was too absolute.** My round-1 doc ended "Callers must
  not discard the bool." The denial cleanup (`mcp.rs:1700`) legitimately discards it —
  deleting a denial record is not a claim, and nothing branches on whether a row was
  there. Narrowed to the claiming caller, with the exemption stated.

## Verified clean (re-derived, not asserted)

- **`append_content`'s header is now correct**, including round-5's attribution fix:
  both `append_content_with_id` sites (`mcp.rs:755`, `:2794`) really are `let _ =`
  inside the detached elicitation task, and both approval-loop `append_content` sites
  (`:1591`, `:1677`) really do log. The constraint name, migration number and the "same
  MAX+1-inside-INSERT" claim all check out.
- **Every approval path pushes exactly one `tool_results` entry** — every push /
  continue / return site mapped: 5 error arms push once then `continue`; the tail pushes
  once; the `is_final` arm pushes once then returns. No path pushes zero or twice.
- **Both SCOPE docs' load-bearing citation holds.** `resolve_unique_tool_use_id` seeds
  from `WHERE message_id = $1` **and mints a fresh `call_<uuid>` on collision**, so
  gpt-oss's constant `"tool_use"` is unique WITHIN a message and recurs only ACROSS
  messages — precisely what both scopes assume. This also rules out the failure mode the
  auditor probed for: a batch-internal duplicate id being wrongly deduped into an
  unpaired tool_use. That cannot happen.
- **`flush_assistant_tool_pair`'s "empty on return" is provable, not aspirational** —
  `insert` requires `pending_ids.remove(id) == true`, and `pending_ids ⊆
  ids(current_tool_uses)` is invariant, so the flush drains every key. The
  `debug_assert!` is sound.
- **Fresh results preserve `name`**, so replacing a placeholder doesn't break Gemini's
  name-based `functionResponse` pairing.
- **Test reality: no undeclared free-riders.** Discriminating power hand-traced —
  reverting the capture guard to `entry().or_insert()` fails both orphan-shadowing
  tests; reverting `batch_start` to `0` fails the older-turn test. The two tests that do
  pass pre-fix each carry an explicit HONEST SCOPE / CHARACTERIZATION doc admitting it.
- **Working tree**: only ` M src-app/server/vendor/pgvector`, whose sole change is an
  untracked `postgresql-18.3.0/` build byproduct of the submodule. No probe edits
  survived (round 5's catch), no source modifications.

**New confirmed findings:** 2
