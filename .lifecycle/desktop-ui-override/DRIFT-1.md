# DRIFT-1.md — implementation vs plan (round 1, in progress)

- **DRIFT-1.1** — verdict: impl-wins — SidebarToggleButton + SidebarHeaderSpacer
  reclassified B→A (seam → `.desktop.tsx` relocation). On reading the real files,
  their divergence is STRUCTURAL (SidebarToggleButton: a different wrapper —
  `TauriDragRegion` overlay + macOS traffic-light offset vs the core
  nativeScroll/headerHidden auto-hide wrapper — different icon set (react-icons/go
  vs lucide), different button styling, different testid; SidebarHeaderSpacer is a
  1-div component the desktop reimplements with drag handlers). A seam would
  eliminate almost no duplication and would need 2 contorted sub-seams. Applying
  the design's OWN decision rule ("`.desktop.tsx` when the whole component
  diverges; `<Seam>` when one element diverges"), both become `.desktop.tsx`
  relocations. Net: seam conversions = 3 (Drawer, SettingsPage, HardwareMonitorButton);
  `.desktop.tsx` relocations = 7 (the original 5 + these 2). PLAN ITEM-6/ITEM-8 +
  TESTS TEST-8/TEST-9 amended accordingly; DEC-10 updated.

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

**Unresolved drifts:** 1
