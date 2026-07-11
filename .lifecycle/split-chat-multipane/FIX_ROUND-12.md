# FIX_ROUND-12 — split-chat-multipane (iteration round 4 blind audit)

Blind audit of the round-4 DELTA (ITEM-44 / FB-9: single-pane pop-out is
desktop-only), on the merged base.

## Blind round (1 fresh agent, diff-only: uncommitted `git diff`)

- **correctness / patterns-conformance / tests-quality** → **[]**. Verified the
  `popoutActionVisible = inPane || isDesktop` truth table across all four cases
  (single-pane/web = hidden; single-pane/desktop = shown; split/web + split/desktop
  = shown — never hides inside a split pane on web). Gate ordering sound: both
  hooks are called unconditionally before any early return (Rules of Hooks), the
  gate sits after `isDesktop` and before `label`. SSR-safe (`typeof window !==
  'undefined'` short-circuits; matches the codebase's existing desktop-detection
  idiom). The rewritten e2e catches BOTH break directions (an always-show break
  fails the single-pane `toHaveCount(0)`; an always-hide break fails the per-pane
  `toBeVisible`); the pop-out-from-split test preserves the independent-tab +
  original-usable coverage AND adds the move-out. No coverage dropped.

Clean; no fix required.

**New confirmed findings:** 0
