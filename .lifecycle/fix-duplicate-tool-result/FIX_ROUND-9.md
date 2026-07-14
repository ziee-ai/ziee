# FIX_ROUND-9 — fix-duplicate-tool-result

Round 9: a full blind re-audit of the round-8 diff. **`[]` — zero findings.** The loop
has converged.

## Nothing to fix

The auditor returned an empty findings array after verifying, independently:

- **The whole doc/label web**, not each doc in isolation — the failure mode of rounds
  3-8. `append_content`'s header and `append_content_with_id`'s doc now agree with each
  other AND with the code: `append_content_with_id`'s only two callers (`mcp.rs:755`,
  `:2794`) really are detached elicitation `tokio::spawn` tasks using `let _ = …`
  (silent drop), while the approval-loop `append_content` sites (`:1591`, `:1677`)
  really do `tracing::error!`. Both `delete_tool_approval` docs agree, and the
  "non-claiming caller" they describe (the denial cleanup, `mcp.rs:1700`) really is the
  only other caller.
- **Every cross-reference resolves**: the `batch_has_result` gate exists
  (`streaming.rs:1823`); the cited integration test exists; the cited prior art
  (`wsl2.rs:2034`) exists; the doc-scan patterns really match migration 124's
  formatting; DEC-1/DEC-2 exist; every cited TEST-N label resolves.
- **Migration 158's header claims are literally true** — 114 does create the bare index,
  124 does add the constraint and its header does contain the quoted sentence, 158 is the
  next free number.
- **One `tool_results` push per approval path** — all six exits traced; no double-push;
  no `?`/`return Err` anywhere in the loop after the claim, so a claimed row cannot be
  stranded by an early bail.
- **Scoping + capture guard** hand-simulated across orphan-before-use,
  orphan-across-flush, duplicate-tool_use-id-in-batch, and duplicate-results shapes;
  `emptied`/`retain` index pairing; `replace_or_collect`'s empty-slice default.
- **Test-discrimination honesty**: TEST-18 and TEST-10 both declare they pass on base;
  TEST-13 declares it pins the decision not the wiring; TEST-9 names the keep-first
  tradeoff instead of letting shape assertions certify it; TEST-5/TEST-16 genuinely fail
  pre-fix (traced against both fixtures).

## Caveat the auditor stated, and how it is covered

It did not compile (the worktree's `src-app/target` is the known-broken symlink from
[[ziee-worktree-build-env]]) and reviewed compile-level concerns by reading instead —
generic `claim_outcome::<()>`, the child test mod's `use super::{…}` of a private
enum/fn, the `emptied` borrow alongside `messages.iter_mut()`, fixture signatures, and
the `unnest(pg_index.indkey)` idiom — finding nothing wrong.

Covered independently on my side: **328 unit + 12 integration tests green**, and a
**from-scratch clean build** (`cargo clean -p ziee`, 74.8 GiB removed, then
`cargo check -p ziee --tests` clean) — satisfying B4, since a warm build can compile
against a stale proc-macro expansion.

## Convergence trace (why this is real, not fatigue)

| Round | Confirmed | Character |
|---|---|---|
| 1 | 7 | **3 HIGH** — cross-turn dedup scoping, bool-discarding claim, claim-failure re-execution |
| 2 | 5 | **2 HIGH** — claim paths abandoning a tool_use (branch-bricking); orphan half-fix |
| 3 | 5 | docs that argued against their own code |
| 4 | 2 | model-facing copy + a denied concurrent caller |
| 5 | 3 | probe edits left in tree; misattributed swallow |
| 6 | 2 | a stale wrapper doc; an over-absolute directive |
| 7 | 1 | a header contradicting itself |
| 8 | 2 | a doc citing a retracted assumption; a dangling label |
| **9** | **0** | **converged** |

Zero correctness/concurrency/security/api-contract defects for four consecutive rounds
(6-9). The tail was entirely documentation, and three of those findings were created by
my own previous fix — the lesson recorded in FIX_ROUND-8: correcting a doc means
re-reading everything that points AT it.

**New confirmed findings:** 0
