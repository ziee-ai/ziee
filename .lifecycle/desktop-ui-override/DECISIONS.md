# DECISIONS.md — resolved up front

### DEC-1: Which override mechanism — build-time file-swap, runtime registry, or hybrid?
**Resolution:** Hybrid. Keep `localOverridePlugin` (build-time whole-file shadow)
UNCHANGED for wholesale component/page/module swaps; ADD a runtime **UI Override
Registry** for in-place sub-element seams. Use file-shadow when the desktop
version differs wholesale; use a seam when desktop differs in one part of a
component the web app still fully owns.
**Basis:** codebase + research — the two survey axes are complementary; the
research recommends "one from each axis," and ziee already ships the file-swap
axis, so the net-new work is the registry (which mirrors the existing chat
`panelRendererRegistry`).

### DEC-2: Registry storage — React context/provider, or a module-level Map?
**Resolution:** A module-level `Map`, populated once at desktop boot, read at
render via a tiny hook. No provider tree.
**Basis:** codebase — platform is FIXED at boot (never changes at runtime), so a
context (which exists to propagate runtime-changing values) is unnecessary
overhead. This is exactly how `panelRendererRegistry` (`Chat.store.ts:105`)
stores renderers.

### DEC-3: Seam API shape?
**Resolution:** Both — a `useOverride(key, Fallback)` hook (returns the winning
component) for logic-heavy call sites, and an `<Override id fallback {...props}/>`
component wrapper for pure-JSX call sites. Both delegate to `resolveOverride`.
**Basis:** convention — mirrors how the kit exposes both hook (`useSurface`) and
component (`<KitSurfaceProvider>`) forms; ergonomics.

### DEC-4: Key naming convention?
**Resolution:** `<module>.<element>` in kebab-case (e.g. `hardware.monitor-button`,
`llm-provider.group-assignment-card`).
**Basis:** convention — matches existing string-keyed registries (chat panel
`type` ids, `data-testid` literals, slot names).

### DEC-5: How are keys/props typed?
**Resolution:** Declaration merging on a base `export interface UIOverrides {}`;
each seam adds `declare module '@/core/overrides' { interface UIOverrides { 'x.y': PropsType } }`.
The value type is the overridable element's props; `registerOverride`/`resolveOverride`
are generic over `keyof UIOverrides`.
**Basis:** codebase — identical to `Slots` (`core/module-system/types.ts:34`),
`RegisteredStores`, and `PanelRendererMap`.

### DEC-6: Fallback semantics when no override is registered?
**Resolution:** Render the Fallback, which IS the extracted original element →
the web bundle (registers nothing) is behaviorally byte-identical to today. A
seam declared but never overridden is a legitimate resting state (tested by
TEST-6).
**Basis:** convention — non-invasive-by-default; the same "resolve→fallback"
pattern as `resolvePanelRenderer` returning null when unregistered.

### DEC-7: Where/when does the desktop register overrides?
**Resolution:** In a desktop module's `initialize()`, invoked by
`loadDesktopModules()` in `desktop/ui/src/main.tsx` BEFORE `ReactDOM.render` —
the same pre-render window that runs `setMultiUserMode(false)`.
**Basis:** codebase — desktop-module init already runs pre-render; registrations
are complete before any routed page (where the exemplar seams live) mounts.

### DEC-8: Is the override mechanism an admin-configurable setting, or fixed?
**Resolution:** FIXED — no settings row, no admin toggle, no migration. Overrides
are CODE (a platform/build architecture concern), not an operational tunable an
admin would flip at runtime. There is no memory/CPU/retention/rate/threshold knob
here.
**Basis:** convention + explicit rationale — the Configurable-settings rule
targets operational tunables; a build-time/platform code seam is categorically
not one. Structured as a typed registry (not magic numbers), so it remains
evolvable without a settings rewrite.

### DEC-9: How is the override surface made discoverable / drift-guarded?
**Resolution:** A generated manifest (`OVERRIDE_MANIFEST.md` + a generated TS key
list) via `ui/scripts/gen-override-registry.mjs`, run in `--check` mode inside
`npm run check` in BOTH workspaces; `--check` FAILS on a registered override
whose key has no declared seam (dead override).
**Basis:** codebase — mirrors `gen-testid-registry.mjs` / `gen-kit-manifest.mjs`.

### DEC-10: Which components become the two exemplar conversions, and by what criteria?
**Resolution:** #1 = `HardwareMonitorButton` (self-contained whole-component
seam; deletes an existing shadow — clean before/after). #2 = an ELEMENT-level
case chosen at Phase-5 implement time by these criteria: (a) the host core
component is non-trivial (so forking-the-whole-file is the real current cost),
(b) desktop needs to change exactly ONE interior element, (c) the host has an
existing gallery surface. Leading candidate: an interior action in
`HeaderBarContainer`/`LeftSidebar`. Final pick recorded as a DRIFT/DEC note in
Phase 5.
**Basis:** codebase — these are the existing whole-file desktop overrides whose
divergence is narrow; picking at implement time avoids guessing the host's shape
before reading it.

### DEC-11: Does converting `HardwareMonitorButton` to a seam change its behavior on either platform?
**Resolution:** No. Web renders the fallback (unchanged browser-popup button);
desktop renders the registered native-window variant (identical to today's
shadow file, minus the duplicated permission/render boilerplate, which now lives
once in the shared fallback wrapper). The `data-testid` on the desktop variant is
preserved so existing desktop e2e keeps passing.
**Basis:** codebase — parity is asserted by TEST-4 (unit) + TEST-5 (e2e).

All decisions above are resolved; no unresolved markers remain.
