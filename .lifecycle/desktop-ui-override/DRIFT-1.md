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

**Unresolved drifts:** 0
