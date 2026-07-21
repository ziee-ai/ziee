# DRIFT-1 — collapse-border-overlay

Implementation audited against PLAN.md / DECISIONS.md after all items were built.

- **DRIFT-1.1** — verdict: impl-wins — **The root cause in PLAN.md was wrong, and
  the reproduction corrected it.** The plan (following the task brief) attributed
  the dimming to the mask's ALPHA RAMP fading low-contrast rings below 288px.
  Measurement refutes that: the kit Card's `ring-1` is a box-shadow with 1px
  SPREAD painted entirely OUTSIDE the border box, the card sits flush with its
  container (`leftInset: 0`), and BOTH `overflow-hidden` and `mask-image` clip to
  the border box (`mask-clip` defaults to `border-box`). The isolation table in
  REPRO.md shows each property ALONE is sufficient to erase the ring, so the ramp
  was never the primary mechanism. PLAN.md's "Root cause" section is superseded by
  REPRO.md; the ITEM list is unchanged because both items were still needed.
- **DRIFT-1.2** — verdict: impl-wins — **ITEM-4 was promoted from defensive polish
  to the PRIMARY fix.** The plan framed the `-mx-0.5 px-0.5` inset as protection
  for prose-level tables/code fences, with ITEM-3's split doing the real work.
  Evidence 3 shows the inset ALONE restores every ring (0 → 25) while still
  clamped. ITEM-3 remains correct and is retained per the explicitly approved
  DEC-1, and it still removes the residual ramp attenuation measured on card 2
  (24 vs 25). Surfaced to the user rather than silently reversing their decision,
  per the audit-vs-user-decision rule.
- **DRIFT-1.3** — verdict: resolved — TESTS.md TEST-2 was amended DURING phase 5,
  before implementation. As originally written it asserted that cards straddle the
  ramp *inside* `collapsible-content` — which becomes false the moment TEST-3
  passes, i.e. a phantom leg that could never hold alongside its sibling. It now
  asserts the property that survives the fix: the cards' combined vertical extent
  exceeds the ramp start. Phase 3 re-gated green after the amendment.
- **DRIFT-1.4** — verdict: resolved — the gallery fixture's trailing prose had to
  be lengthened (8 → 13 copies). Sized against the whole turn it measured 368px
  once the cards were hoisted out — under the 384px clamp — so the message stopped
  collapsing entirely and the surface silently stopped covering the bug. Recorded
  in REPRO.md; TEST-2's `data-collapsed === 'true'` assertion now guards it.
- **DRIFT-1.5** — verdict: resolved — `npm run gen:testid-registry` was run per
  ITEM-5 and its output was DISCARDED. It writes into the `sdk` SUBMODULE
  (`sdk/packages/kit/src/testIds.generated.ts`, per `gallery.config.json`
  `testidOut`) and produced 24 insertions / 12 deletions of unrelated
  split-pane / conversation-picker / notification ids — pre-existing drift between
  the submodule's committed registry and the current tree, owned by the sdk repo.
  This diff adds NO new `data-testid` (it reuses `collapsible-toggle`,
  `collapsible-content`, `thinking-card`, `data-slot="card"`), so no regen is
  warranted. `sdk` was restored to a clean state and is absent from the diff.
  A stray `100644 → 100755` mode flip on `sdk/packages/gallery/scripts/cli.mjs`,
  caused by `npm install`, was reverted for the same reason.
- **DRIFT-1.6** — verdict: none — ITEM-3's two PLAN_AUDIT refinements were both
  implemented as specified: `classifyNode` tags over EVERY consumed block rather
  than the first (defensive against a future `contentSpan`), and the
  `CollapsibleBlock` wrapper is skipped entirely when the prose suffix is empty
  (`offerCollapse && clampedNodes.length > 0`), per DEC-4.
- **DRIFT-1.7** — verdict: none — `gallery.config.json` gained
  `chat-collapse-borders.spec.ts` in `visualSpecs`. Not named in PLAN.md's *Files
  to touch*, but required: a new spec file is not part of the gate unless
  registered there. Consistent with the plan's intent, not a scope change.

- **DRIFT-1.8** — verdict: impl-wins — **ITEM-5's "regenerate the gallery
  registries" half was DROPPED, and PLAN.md/TESTS.md amended.** Running
  `gen:gallery-coverage` + `gen:state-matrix` produced a 1768-deletion diff that
  removed ~63 `components/ui/kit/*` surfaces — pre-existing drift, because the kit
  moved into the `sdk` submodule during the F1/F2 migration and
  `src/components/ui/` no longer exists, but the committed registries were never
  refreshed. It also broke `tsc`, since the hand-maintained `coverage.ts` /
  `stateCoverage.ts` still carry those keys under a
  `satisfies Record<GallerySurface, Coverage>` constraint. This diff adds no
  component file and no `data-testid`, so the generators emit nothing new for it —
  the entire regen was someone else's cleanup. Registries reverted to the base
  state; only the two `coverage.ts` reason strings remain. Verified afterwards that
  `tsc` is clean and all 6 new specs still pass without the regen.
- **DRIFT-1.9** — verdict: none — `npm run check` is RED on the base commit
  (`origin/khoi`) at `check:testid-registry`, verified by detaching to
  `origin/khoi` and re-running it there. Same SDK-migration cause; the stale file
  is `sdk/packages/kit/src/testIds.generated.ts`, INSIDE the submodule, so it
  cannot be fixed from a PR into `khoi`. Escalated to the user as an explicit
  scope choice rather than silently absorbed or silently ignored; they chose to
  ship the focused fix and document the pre-existing red. Recorded in
  TEST_RESULTS.md with the verification transcript.

## Environment deviations (not code drift, recorded for reproducibility)

- The gallery runs on **port 1437**, not the configured 1420: a foreign server
  (bound IPv6-only on `[::1]:1420`, serving a different tree) already held the
  default. It was NOT killed — it is not this worktree's. Every gate run passes
  `GALLERY_PORT=1437`. Note `gate:ui` reuses an already-running server via an HTTP
  probe, so running it on the default port here would have silently exercised the
  WRONG tree.
- 10 pre-existing `npm run test:unit` failures (all `*.store.test.ts` plus
  `runTimeline`) were confirmed byte-identical on a clean stash of the base commit,
  so they are not attributable to this change.
- The backend e2e (`collapse-long-message.spec.ts`) initially failed at
  `loginAsAdmin` with an S3 `AccessDenied` page. Cause: **MinIO occupies port 9000
  on this host**, which is the harness's default `BASE_VITE_PORT`, so the browser
  talked to MinIO instead of Vite. Confirmed environmental by reproducing the
  identical failure on a detached `origin/khoi`. Resolved with the harness's OWN
  documented cross-worktree knobs — `ZIEE_E2E_BASE_VITE_PORT=9600`,
  `ZIEE_E2E_BASE_BACKEND_PORT=9700`, `ZIEE_E2E_LOCK_DIR=/tmp/ziee-test-locks-collapse`
  — NOT by editing the shared harness (B3). It passes with those set.
- Docker access needed `sg docker -c '…'`: the `docker` group contains `khoi` but
  this shell session predates the membership.

**Unresolved drifts:** 0
