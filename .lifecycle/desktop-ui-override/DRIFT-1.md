# DRIFT-1.md — implementation vs plan (round 1, in progress)

- **DRIFT-1.1** — verdict: resolved — SidebarToggleButton + SidebarHeaderSpacer
  reclassified B→A and RETAINED as tier-1 desktop-tree shadows (no change), not
  converted to seams and not relocated. Reasons: (1) their divergence is
  STRUCTURAL, not element-level (SidebarToggleButton: different wrapper —
  `TauriDragRegion` overlay + macOS traffic-light offset — different icon set,
  styling, testid; SidebarHeaderSpacer: a 1-div component reimplemented with drag
  handlers) — a seam eliminates ~no duplication and would need contorted sub-seams;
  (2) they are consumed RELATIVELY by core `LeftSidebar` (`./SidebarToggleButton`),
  and tier-2 `.desktop.tsx` only intercepts `@/` imports — relocating them would
  force rewiring core consumers to `@/` for zero duplication benefit. The existing
  tier-1 shadow already handles them correctly. Net: seam conversions = 3 (Drawer,
  SettingsPage, HardwareMonitorButton); `.desktop.tsx` relocations = 5 (done); 2
  structural overrides retained as tier-1 shadows. PLAN ITEM-6/ITEM-8 + TESTS
  TEST-8 + DEC-10 amended.

- **DRIFT-1.4** — verdict: resolved — `.desktop.tsx` co-location has a barrel
  caveat surfaced during the AuthGuard relocation: a core barrel that RELATIVELY
  re-exports (`export { X } from './X'`) yields the CORE version in the desktop
  build, because tier-2 resolution only fires for `@/` specifiers. Fix pattern: the
  desktop keeps (or adds) a barrel shadow that re-exports via `@/modules/.../X` so
  the resolver picks the `.desktop` file. Documented in the `.desktop.tsx` section
  of the docs (ITEM-12) as a required consumer rule.

- **DRIFT-1.2** — verdict: resolved — the Seam primitive lives in `Override.ts`
  (not `.tsx`) because the core `node --test` runner strips types but does not
  transform JSX; the component is JSX-free (`createElement`) so `.ts` is correct
  and importable by unit specs. No behavior change.

- **DRIFT-1.3** — verdict: resolved — TEST-4 (resolver) lives under
  `desktop/ui/src/core/local-override-resolver.test.ts` (not `plugins/…`) because
  vitest's include glob is `src/**`; it imports the pure `resolveOverridePath`
  from `../../plugins/`. The testid-unique `.desktop`-shadow-awareness assertion
  moved into the relocation work (it only matters once real `.desktop.tsx` files
  with testids exist).

- **DRIFT-1.5** — verdict: resolved — memory/module reclassified from `.desktop.tsx`
  relocation BACK to a tier-1 desktop-tree module. Surfaced by the relocation agent:
  `module.tsx` files are discovered by `import.meta.glob`, which BYPASSES the `@/`
  resolver — so a core-tree `module.desktop.tsx` is found by neither
  `desktop-loader.ts` (globs the desktop tree, from which the file was removed) nor
  the core `loader.ts` (globs the literal `module.tsx`, which `module.desktop.tsx`
  does not match). Relocating it orphaned the `memory-desktop` module (a real
  regression). Reverted to `desktop/ui/src/modules/memory/module.tsx` (its original,
  working, glob-discovered location). Net: `.desktop.tsx` relocations = 4 (AuthGuard,
  LeftSidebar, HeaderBarContainer, ProviderGroupAssignmentCard); memory/module +
  the 2 sidebar overrides retained as tier-1 shadows. GENERAL RULE (documented in
  UI_OVERRIDES.md + a codemod guard candidate): `.desktop.tsx` co-location works
  only for `@/`-imported files, NEVER for glob-discovered `module.tsx`.

- **DRIFT-1.6** — verdict: impl-wins — Drawer + SettingsPage reclassified B→A on
  close reading (the triage's element-level call was optimistic). Both diverge
  STRUCTURALLY, not in a single element: desktop Drawer differs in overlay styling,
  header LAYOUT (close-right + window-drag vs close-left + a11y), chrome-reserve
  insets, footer padding, and resize max-width — a seam would need 5+ contorted
  sub-seams and can't cleanly capture the desktop chrome/inset needs (which core
  can't compute — it can't import desktop platform helpers). Desktop SettingsPage
  likewise renders a deliberately different (flat, single-user) menu and drops the
  permission-filter/403/help-footer BY DESIGN (single-admin desktop), not as drift.
  Resolution: BOTH retained as tier-1 desktop-tree shadows (their current, working
  location) — NOT seams, NOT relocated. Seam exemplar remains HardwareMonitorButton
  (1 clean element-level conversion); the infra's element-level capability is proven
  by it, and most existing overrides are honestly structural (file-swap). ITEM-8
  seams = 1; PLAN/TESTS/DEC amended.

- **DRIFT-1.7** — verdict: resolved — ITEM-9 (Drawer drift) is executed IN the
  retained tier-1 Drawer shadow (not during a seam conversion): restore core's
  `higherLayerOpen` stacking guard (a real bug — closing a dialog stacked above the
  drawer also dismissed the drawer) + `titleText`/node-title `Dialog.Title` a11y +
  the `data-testid` override + the `data-slot="layout-drawer"`/`layout-drawer-content`
  markers the stacking-guard query depends on. Swipe-to-close is deliberately NOT
  ported (touch-drag-to-close is low-value on a mouse/trackpad desktop and the
  stacking guard is the actual regression); noted here as an intentional omission.

**Unresolved drifts:** 0
