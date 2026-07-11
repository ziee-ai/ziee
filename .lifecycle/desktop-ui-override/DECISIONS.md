# DECISIONS.md — resolved up front

### DEC-1: Which override mechanism — build-time file-swap, runtime registry, or hybrid?
**Resolution:** Hybrid, gone aggressive. Keep `localOverridePlugin` (whole-file
shadow) AND add `.desktop.tsx` co-location for whole-file cases; ADD a runtime UI
Override Registry + `<Seam>` primitive for element-level cases; ADD a codemod to
migrate existing shadows.
**Basis:** codebase + research + user — the two axes are complementary; the user
selected the aggressive tooling path (codemod + `.desktop.tsx`) in design review.

### DEC-2: Registry storage — React context/provider, or a module-level Map?
**Resolution:** Module-level `Map`, populated once at desktop boot, read at render.
No provider tree.
**Basis:** codebase — platform is fixed at boot; mirrors `panelRendererRegistry`
(`Chat.store.ts:105`).

### DEC-3: Seam API shape?
**Resolution:** A `<Seam id props>{fallback}</Seam>` wrap-in-place component
(children are the fallback — no `DefaultFoo` extraction needed) as the primary
form, plus a `useOverride(key, Fallback)` hook for logic-heavy sites. Both delegate
to `resolveOverride`.
**Basis:** convention + user "aggressive / low-instrumentation" steer — wrapping in
place is the least-ceremony way to declare a seam.

### DEC-4: Key naming convention?
**Resolution:** `<module>.<element>` kebab-case (e.g. `hardware.monitor-button`,
`layout.drawer-header`).
**Basis:** convention — matches chat panel `type` ids, `data-testid`, slot names.

### DEC-5: How are keys/props typed?
**Resolution:** Declaration merging on `interface UIOverrides {}`; value = the
overridable element's props; `register`/`resolve` generic over `keyof UIOverrides`.
The codemod GENERATES the declaration so there is no manual typing ceremony.
**Basis:** codebase — identical to `Slots`/`RegisteredStores`/`PanelRendererMap`.

### DEC-6: Fallback semantics when no override is registered?
**Resolution:** Render the fallback (the `<Seam>` children = original markup) → web
is byte-identical to today. A declared-but-unoverridden seam is a valid resting
state (tested).
**Basis:** convention — non-invasive-by-default; mirrors `resolvePanelRenderer`
returning null on miss.

### DEC-7: Where/when does the desktop register overrides?
**Resolution:** In a desktop module `initialize()`, invoked by `loadDesktopModules()`
in `main.tsx` BEFORE `ReactDOM.render` — same pre-render window as
`setMultiUserMode(false)`.
**Basis:** codebase — desktop-module init runs pre-render; the exemplar seams live
in routed pages that mount well after boot.

### DEC-8: Is the override mechanism an admin-configurable setting, or fixed?
**Resolution:** FIXED — no settings row / admin toggle / migration. Overrides are
CODE (platform/build architecture), not an operational tunable. No
memory/CPU/retention/rate/threshold knob exists here.
**Basis:** convention + explicit rationale — the Configurable-settings rule targets
operational tunables; a build/platform code seam is categorically not one.

### DEC-9: How is the override surface discoverable / drift-guarded?
**Resolution:** A generated `OVERRIDE_MANIFEST.md` + generated key list via a
`seam check` / `gen-override-registry.mjs`, run `--check` inside `npm run check` in
BOTH workspaces; `--check` fails on a dead override (registered key, no declared
seam) and on an orphaned `*.desktop.tsx` (no core sibling).
**Basis:** codebase — mirrors `gen-testid-registry.mjs`.

### DEC-10: Which existing shadows convert to seams vs relocate vs stay?
**Resolution:** Per the triage, REFINED at implement time (DRIFT-1.1/1.5/1.6) — 1
seam (`HardwareMonitorButton`, a genuine element-level divergence); 4 class-A →
`.desktop.tsx` co-location (`AuthGuard`, `LeftSidebar`, `HeaderBarContainer`,
`ProviderGroupAssignmentCard`); retained as tier-1 desktop-tree shadows:
`Drawer` + `SettingsPage` (structural divergence — many elements differ + desktop
chrome/inset needs core can't compute; Drawer additionally gets a drift/bug fix,
DRIFT-1.7), `memory/module` (`module.tsx` is glob-discovered), `SidebarToggleButton`
+ `SidebarHeaderSpacer`; 8 class-C infra → unchanged. The seam mechanism's
element-level capability is proven by HardwareMonitorButton; the honest finding is
that most existing desktop overrides are STRUCTURAL and correctly use file-swap.
**Basis:** codebase — a seam removes duplication only for genuine element-level
divergence; structural/whole-component overrides stay as file-swaps (tier-1 shadow
or tier-2 `.desktop.tsx`).

### DEC-11: Does converting a shadow change platform behavior?
**Resolution:** No, except the intentional Drawer fix (DEC-15). Web renders the
unchanged fallback; desktop renders a variant identical to today's shadow minus the
duplicated boilerplate. Desktop `data-testid`s preserved so existing desktop e2e
keeps passing.
**Basis:** codebase — asserted by TEST-8 (unit parity) + TEST-9 (e2e).

### DEC-12: Full auto-migration codemod, or scaffold + hand-convert?
**Resolution:** Full auto-migration codemod (ts-morph): `migrate` AST-diffs a
shadow vs its core sibling and rewrites both sides + deletes the shadow; `add`
scaffolds a new seam. **Mitigation (mandatory):** codemod output is
HUMAN-REVIEWED before commit, is fixture/golden-tested (TEST-5), and per-seam
parity is asserted (TEST-8) — the codemod accelerates the edit, it does not
blind-ship it.
**Basis:** user — chose the aggressive codemod in design review; I flagged the risk
on the 5 subtle files and the mitigation is the review + tests.

### DEC-13: `.desktop.tsx` co-location in the core tree, or leave class-A in the desktop tree?
**Resolution:** Co-locate — relocate the 5 class-A shadows to
`ui/src/<path>.desktop.tsx`. Accepted tradeoff: Tauri-importing files now live in
the web workspace; they are never bundled by web vite and are EXCLUDED from web
`tsconfig`/`biome` via `**/*.desktop.*`, so they are inert there while the desktop
workspace typechecks + bundles them.
**Basis:** user — chose co-location in design review; the exclude mechanism
neutralizes the cross-workspace dependency concern I raised.

### DEC-14: `.desktop.tsx` resolution precedence vs the existing desktop-tree shadow?
**Resolution:** For a desktop `@/foo` import the resolver probes, in order: (1)
desktop-tree `desktop/ui/src/foo.*` (existing behavior, unchanged), (2) core-tree
`foo.desktop.*` (new), (3) core-tree `foo.*` (core base). Both override mechanisms
coexist; class-A files move to tier (2), desktop-only modules keep using tier (1).
**Basis:** codebase — preserves all existing resolutions; the new tier slots
between the desktop-tree shadow and core base.

### DEC-15: Reconcile Drawer drift during conversion?
**Resolution:** Yes — restore core's swipe-to-close + `higherLayerOpen` stacking
guard that the desktop shadow silently dropped; the desktop Drawer seam overrides
ONLY the header/inset/traffic-light elements, so swipe + stacking flow from the
shared fallback. This is a bug FIX made possible by the conversion, not propagated.
**Basis:** codebase + user (accepted aggressive path) — asserted by TEST-6.

### DEC-16: Codemod AST library?
**Resolution:** Prefer `ts-morph`; confirm it is (or can be) a devDependency in
Phase 5. If it cannot be added, fall back to the TypeScript compiler API directly
(no new dep). Either way the codemod is a build-time `.mjs` dev script, never
shipped in a binary.
**Basis:** convention — build-time-only tooling, resolved to a concrete fallback so
implementation does not stall.

All decisions above are resolved; no unresolved markers remain.
