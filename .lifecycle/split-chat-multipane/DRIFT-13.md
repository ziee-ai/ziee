# DRIFT-13 — split-chat-multipane (ITEM-71: split header ↔ app header consistency)

Reconciliation for ITEM-71 (FB-18 regression fix — the split per-pane header diverged
from the single-pane `HeaderBarContainer`, and the sidebar-collapse toggle was
unclickable in split).

- **DRIFT-13.1** — verdict: impl-wins→fixed — the split's compact header was hand-rolled
  (`h-11`, static `px-3`) instead of matching/reusing `HeaderBarContainer` (50px,
  `paddingLeft: 118/48/12` to clear the toggle + macOS traffic lights). This dropped the
  encoded constraints and shipped visible regressions (height, left padding) the author
  couldn't see without running in the real chrome. Fixed by a shared `useHeaderLeftInset`
  hook (single source of truth for both) + 50px height. Same lesson as FB-17/DRIFT-12.2:
  understand + reuse the sibling, don't re-derive.

- **DRIFT-13.2** — verdict: resolved — the sidebar-collapse toggle was unclickable in
  split: the focused pane's `z-10` (focus-ring lift) equalled the fixed `z-10` toggle,
  and (main-content is `relative` z-auto = no stacking context) the pane, later in the
  DOM, painted over it. Fixed by lowering the focus ring to `z-[5]` (above sibling panes
  at z-auto, below the toggle). Proven by TEST-108's REAL `.click()` (Playwright's
  actionability check fails on a covered target — a synthetic dispatch could not catch it).

- **DRIFT-13.3** — verdict: none — refactored `HeaderBarContainer` core + desktop to
  consume the shared hook (no behavior change for them — the hook returns the identical
  value), so the app header and the split header can't drift. The new `.desktop.ts` hook
  is a registered override seam (OVERRIDE_MANIFEST regenerated).

**Unresolved drifts:** 0
