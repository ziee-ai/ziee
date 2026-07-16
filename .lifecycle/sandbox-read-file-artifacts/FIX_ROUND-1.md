# FIX_ROUND-1

Merged LEDGER.jsonl → fixed every `confirmed` finding → re-ran a blind round.

## Confirmed findings fixed

- **list_files dedup (state-management/correctness/i18n — 3 auditors).** `list_files`
  now seeds a `seen` set from workspace names AND this user's `ctx.files` attachment
  names, and pushes each model-authored artifact only if `seen.insert(name)` succeeds
  — so a name shadowed by a workspace file OR an attachment is skipped, and two
  same-named artifacts collapse to one row. Comment rewritten to describe the real
  workspace→attachment→artifact precedence (dropped the overstated "no drift").
  Covers the artifact-vs-artifact AND artifact-vs-attachment collision classes.
- **AMBIGUOUS_FILENAME (artifact) wording (i18n).** Reworded to
  "{filename} matches {n} tool-produced files with the same name … read_file
  resolves artifacts by name only and cannot tell them apart; read a specific one
  by its id using the files read_file tool." Removed the imprecise
  "`files` MCP … (from its list_files)" phrasing.
- **TEST-5 paper-green (tests-quality, med).** Strengthened: after asserting
  workspace-first for `dup.txt`, it now also reads `artifact-only.txt` (no workspace
  shadow, not an attachment) — which resolves ONLY via the new fallback, so the test
  fails if the fallback is reverted.
- **TEST-9 weak assertion (tests-quality).** Now asserts the message contains
  "cannot tell them apart" AND "tool-produced files" — substrings unique to the
  AMBIGUOUS branch (FILE_NOT_FOUND / attachment-ambiguity use different wording), so
  it positively distinguishes the ambiguity path from a not-found.
- **enabled_server dir leak (tests-quality).** Now uses ONE shared idempotent
  placeholder rootfs dir instead of a per-test `-<uuid>` dir.
- Added a TEST-7 assertion that two same-named artifacts (`twins.txt`) collapse to
  exactly one list row (covers the list_files dedup fix).

## Findings accepted as-designed (rejected, with rationale — see LEDGER)

- Two `performance` findings (list_files always queries; no LIMIT on hydration) —
  bounded by conversation artifact count and identical to the reviewed files-MCP
  manifest query; no cheap skip; pagination would diverge from the sibling.
- `i18n` divergent attachment-vs-artifact AMBIGUOUS remediation — each is locally
  correct (attachments are bind-mounted → `cat`; artifacts are not → files-MCP id).

## Re-audit (blind round 1)

Two fresh blind auditors (correctness/state/security + tests-quality) reviewed the
post-fix diff. Tests-quality: 0 defects (all strengthened tests confirmed genuine —
each fails on revert; asserted substrings match the real message strings).
Correctness/state: **1 new confirmed** low finding — in `list_files`, the collapsed
same-named-artifact row's `size` was nondeterministic because `get_by_ids_and_user`
(`WHERE id = ANY(...)`) does not preserve order.

**New confirmed findings:** 1
