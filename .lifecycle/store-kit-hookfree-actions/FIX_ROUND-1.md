# FIX_ROUND-1 — store-kit-hookfree-actions

## Confirmed findings from the blind audit (LEDGER.jsonl) and their fixes

1. **[patterns-conformance, low] Stale `__state` comments at ~13 swept sites.**
   The `.__state` code was rewritten to `$`/direct-action, but 14 CODE COMMENTS
   still described the removed alias (they used bare `__state`, no leading dot,
   so the sweep regex — which matched `.__state` — skipped them). Fixed all 14:
   - Action-call sites (file/chat-extension `extension.tsx`, `FilePreviewList`,
     `InlineFilePreview`, `FileUploadArea`, `FileVersionBar`, mcp `extension.tsx`
     `addToolCall`) → rewritten to "X is an action — callable directly (hook-free)".
   - Snapshot-read sites (assistant `extension.tsx`, mcp `extension.tsx` ×2,
     `HubPage`, hub/mcp `module.tsx`, `useHubModelDownloadGate`,
     `SkillDetailDrawer`) → rewritten to "read via `$` snapshot".
   Each rewritten comment was verified against its adjacent code (re-audit
   round 2 independently confirmed every comment matches the code).
   One false match (`SEEDED_GALLERY_PLAN.md` `surface__state__theme`, a gallery
   naming token) was correctly left untouched.

   Follow-on: the mcp `extension.tsx` comment grew 1→2 lines, shifting recorded
   line numbers, so `npm run gen:state-matrix` was re-run for the `ui` workspace
   (6-line line-number delta in the generated matrix); `check:state-matrix` green.
   AUDIT_COVERAGE.tsv gained rows for the ui-generated matrix files.

## Re-audit (full blind round, round 2)

A fresh blind reviewer (diff-only context, incl. the comment fixes) re-checked
every rewritten site, the proxy get-trap, the type changes, the guardrail, and
the test/loader/stubs, with an explicit pass over comment-vs-code accuracy.
Verdict: **clean — no real defect found.** Confirmed: no action turned into an
un-invoked `.$` read, no state field left as a bare hook-firing read in a
handler, `$` snapshots correctly re-read post-`await`, render-context `$` uses
are intentional non-reactive snapshots (semantics identical to the old
`__state`), all updated comments match code, no live `.__state` remains in
source, desktop shares the single proxy via tsconfig path mapping.

The reviewer also noted a `hub-settings-sync.spec.ts` deletion — verified this is
NOT in this diff (absent from both origin/main and HEAD; `git diff
origin/main...HEAD --name-status` shows zero deletions), so it is a non-finding.

## Post-fix verification

- `npm run check` — PASS in BOTH `ui` and `desktop/ui` (after matrix regen).
- `test:unit` — 13/13 PASS.
- No residual live `.__state` in source; guardrail flags any reintroduction.

**New confirmed findings:** 0
