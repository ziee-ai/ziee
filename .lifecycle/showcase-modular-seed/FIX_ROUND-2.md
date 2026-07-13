# FIX_ROUND-2 — merge-with-origin/main reconciliation re-audit (Phase 7, iteration)

After the feature was 9/9, `origin/main` advanced 56 commits to tip **f60683384**.
This branch restructured the central gallery aggregators main kept editing, so the
merge required reconciliation (not a fast-forward). A fresh BLIND diff-only agent
re-audited the merge delta.

## Reconciliation performed
- **seededSurfaces.tsx** conflict → kept the branch's thin aggregator (`--ours`);
  main's 5 NEW seeded surfaces (`seeded-recent-convos-{loaded,error,loading-more}`,
  `seeded-conversation-list-long{,-narrow}`) re-homed into `chat/gallery.tsx`
  (byte-identical entries; `./ConversationListLongDemo` → `@/dev/gallery/…`;
  `mkRecentConvos` helper carried over). Main also RENAMED the `ChatHistory` recent
  fields (`isInitialized`→`recentInitialized`, `loading`→`recentLoading`, …); the
  re-homed entries use main's current names (a stale name would silently mis-render
  since setState is cast `as any`) — and my kept `recent-convos-loading`/`-empty`
  were replaced with main's updated versions.
- **overlay-allowlist.json** conflict → UNION: the 3 drawers this branch WIRED
  (ScheduledTaskFormDrawer/KnowledgeBaseFormDrawer/UploadModelDrawer) correctly
  stay REMOVED (they'd GC-fail as stale); main's new inline `ScheduledTaskCard`
  confirm correctly KEPT.
- **Generated files** (testIds/state-matrix/gallery-coverage/overlay-registry/
  seed-registry, both workspaces) → REGENERATED from the merged source.
- Seed baseline regenerated (155 → **163** = migrated + 3 wired overlays + 5 merge).

## Blind re-audit result (audit/ledger-merge.jsonl)
All 4 checks CLEAN:
1. All 5 `recent-convos-*` entries use the correct renamed `recent*` `ChatHistory`
   fields (the highest-risk item — confirmed).
2. Main's 5 new entries re-homed verbatim (byte-identical to origin/main).
3. Overlay-allowlist union correct (wired drawers removed, ScheduledTaskCard kept).
4. Full-tree slug diff (origin/main vs HEAD, all ui/src `*.ts[x]`): **all 160 main
   gallery slugs present on HEAD, ZERO lost**; HEAD adds only the 3 branch-wired
   overlay slugs (+ the per-module migration).

## Post-merge verification (on top of f60683384)
- `npm run check` PASS (ui + desktop/ui); tsc clean both.
- `gate:ui --skip-visual`: **181/181 (ui) + 47/47 (desktop)** runtime-clean.
- e2e: seed-parity (163/163 register), newly-seeded, gap — all green.
- B6 strip test: seed gate passes with `.lifecycle` moved aside (reads the
  permanent `GALLERY_SEED_EXCEPTIONS.md`).

**New confirmed findings:** 0
