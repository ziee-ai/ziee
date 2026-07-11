# Desktop UI Override Infrastructure

The desktop app (`src-app/desktop/ui`) reuses the web UI (`src-app/ui`) via the
`@/…` alias. This doc is how you customize UI for the desktop build **at the right
granularity** — one element, one file, or a whole module — without duplicating
code you don't need to.

There are **three tools**. Pick by how much of a component actually differs:

| You want to override… | Tool | Where the override lives |
|---|---|---|
| **One element** inside a shared component (a button, a header, an icon) | **`<Seam>`** (runtime registry) | core wraps the element; desktop registers a replacement |
| A **whole component / page / module** that diverges structurally | **`.desktop.tsx`** (build-time, tier-2) | `ui/src/<path>.desktop.tsx` next to the core file |
| A whole component that also has **relative-import consumers** or is desktop-only | **tier-1 shadow** (build-time) | `desktop/ui/src/<same-path>` |

The resolver (`desktop/ui/plugins/vite-plugin-local-override.ts`) resolves a
desktop `@/foo` import in this precedence order:

1. **tier 1** — `desktop/ui/src/foo.*` (desktop-tree shadow)
2. **tier 2** — `ui/src/foo.desktop.*` (co-located whole-file override)
3. **tier 3** — `ui/src/foo.*` (the shared web implementation)

---

## 1. `<Seam>` — override one element

**When:** the web component is mostly shared and desktop changes ONE interior
element. This is the only tool that avoids forking the whole file.

**Core (web)** — wrap the element; its children are the fallback, and declare the
seam's key → props contract via declaration merging:

```tsx
import { Seam } from '@/core/overrides'

declare module '@/core/overrides' {
  interface UIOverrides {
    // `<module>.<element>`, kebab-case. Value = the props the override receives
    // (Record<string, never> when it takes none).
    'hardware.monitor-button': Record<string, never>
  }
}

export function HardwareMonitorButton() {
  const canMonitor = usePermission(Permissions.HardwareMonitor)
  if (!canMonitor) return null            // shared gate — lives ONCE, in core
  return (
    <Seam id="hardware.monitor-button">
      {/* fallback = the original web markup, unchanged */}
      <Button onClick={openBrowserPopup}>Monitor</Button>
    </Seam>
  )
}
```

**Desktop** — drop a file under `desktop/ui/src/modules/desktop-base/overrides/`
exporting `register()` (auto-discovered + called pre-render by
`overrides/index.ts`):

```tsx
// overrides/hardware-monitor.tsx
import { registerOverride } from '@/core/overrides'

function DesktopHardwareMonitorButton() {
  return <Button onClick={openNativeWindow}>Monitor</Button>   // only what differs
}

export function register() {
  registerOverride('hardware.monitor-button', DesktopHardwareMonitorButton)
}
```

- The web bundle registers nothing → every `<Seam>` renders its fallback → web is
  **byte-identical** to before the seam existed.
- Registration runs synchronously in `main.tsx` before `ReactDOM.render`, so the
  override is present on the component's first paint.
- `props` on `<Seam>` are forwarded to the override; the type is required when the
  seam declares any prop, optional when it declares none.
- `useOverride('key', Fallback)` is the hook form for logic-heavy call sites that
  want the component reference.

## 2. `.desktop.tsx` — override a whole file, co-located

**When:** the whole component/page/module differs structurally (a seam would
eliminate ~no duplication). Put the desktop version NEXT TO the core file:

```
ui/src/modules/layouts/app-layout/components/LeftSidebar.tsx          ← web
ui/src/modules/layouts/app-layout/components/LeftSidebar.desktop.tsx  ← desktop build picks this (tier 2)
```

The web build never imports the `.desktop.tsx` (nothing references a `.desktop`
specifier), and the web workspace excludes `**/*.desktop.{tsx,ts}` from
`tsconfig`/`biome`, so Tauri-importing files are inert there. The **desktop**
workspace typechecks + bundles them (its tsconfig includes `../../ui/src` and it
has the Tauri deps).

> **⚠️ Barrel caveat (important).** Tier-2 resolution only fires for **`@/`**
> imports. A core barrel that RELATIVELY re-exports — `export { X } from './X'` —
> resolves `./X` to the CORE file even in the desktop build, bypassing
> `X.desktop.tsx`. If a `.desktop.tsx` override isn't taking effect, a relative
> re-export is why. Fix: have the desktop keep a barrel shadow that re-exports via
> the alias — `export { X } from '@/modules/.../X'` — so the resolver picks the
> `.desktop` file (see `desktop/ui/src/modules/auth/index.ts`).

## 3. tier-1 shadow — the original whole-file mirror

**When:** desktop-only modules, or a whole-file override whose core consumers
import it relatively (so tier-2 wouldn't apply). Put the file at the mirrored path
in the desktop tree: `desktop/ui/src/<same-path>`. This is the historical
mechanism; it still works and has the highest precedence.

Examples retained as tier-1 shadows: `SidebarToggleButton`, `SidebarHeaderSpacer`
(structural divergence + relative-import consumers).

---

## The decision rule

**`<Seam>` when ONE element diverges; a whole-file override (`.desktop.tsx`
preferred, tier-1 shadow when relative consumers force it) when the WHOLE
component diverges.** Don't reach for a seam to save 2 lines, and don't fork a
300-line file to change one button.

## Discoverability + drift guard

`OVERRIDE_MANIFEST.md` (in `ui/src/core/overrides/`, generated by
`scripts/gen-override-registry.mjs`) lists every seam and every `.desktop.tsx`.
`npm run check` runs `check:override-registry`, which fails on:
- a `registerOverride('key')` whose key has no declared seam (a **dead override**), and
- an orphaned `*.desktop.tsx` with no core sibling.

Regenerate with `npm run gen:override-registry` after adding/removing a seam.
