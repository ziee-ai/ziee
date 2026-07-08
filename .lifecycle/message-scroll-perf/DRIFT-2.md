# message-scroll-perf — DRIFT-2 (phase-8 e2e finding)

A drift surfaced by the phase-8 e2e run (not the phase-5 self-audit): the ITEM-5
overscan value chosen in the plan/DECISIONS regressed a pre-existing invariant.

- **DRIFT-2.1** — verdict: impl-wins — **overscan 4 → 8.** DEC-5 set `overscan: 4`
  to reduce heavy off-screen mounts. The `lazy-load-messages` e2e (the
  reverse-infinite-scroll anchor invariant, green on main) then FAILED
  reproducibly: after older messages prepend, the viewport drifted ~120px (vs the
  <80px "anchored" tolerance). Root cause: with overscan 4, fewer off-screen rows
  ABOVE the viewport are rendered/measured, so the prepend anchor-restore's
  `getOffsetForIndex` leans on the coarse per-message estimate (which for these
  short messages is the 140 floor vs a real ~65–80px), inflating the restore
  target. Ruled out: the `anchorRestoreNeeded` guard (the 120px drift is ≫ its 2px
  threshold, so it never triggers — behaviorally identical to the original
  restore) and the estimate itself (=140 floor for these short messages, identical
  to main's constant). The single behavioral delta vs main was overscan.
  **Resolution:** reverted `overscan` to 8 (main's value). `lazy-load-messages`
  then passes (<80px); the mounted-count assertions still hold
  (`virtualize-messages` <20, geometry-stability <24 — at the initial bottom/top
  positions only one side of overscan applies, so ~14 rows mount); DEC-5 amended.
  ITEM-5 therefore ships as a no-op relative to main (overscan unchanged) — the
  perf gains come from ITEM-1/2/3, not overscan.

**Unresolved drifts:** 0
