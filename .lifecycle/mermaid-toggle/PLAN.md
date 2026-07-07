# PLAN — Mermaid code⇄render toggle

Implements backlog item **7(a)** of `src-app/ui/docs/AFFORDANCE_MATRIX.md`
(closes gaps **G1** source⇄render toggle + **G2** copy-source; adds a
download-SVG rider). Mermaid code fences in chat markdown render the diagram by
default and expose a toggle to view/copy the source, plus a copy-source button
and a download-SVG button.

## Context / mechanism

Streamdown 2.5.0 renders ` ```mermaid ` fences internally in its `code`
component (`[data-streamdown="mermaid-block"]`) with NO source toggle. Streamdown
exposes a first-class extension point — `plugins.renderers` (`CustomRenderer[]`):
its `code` component resolves a per-language custom renderer (`ao(lang)`) BEFORE
the built-in mermaid path, so registering `{ language: 'mermaid', component }`
fully replaces the built-in mermaid rendering with ours. We render the diagram
via the `mermaid` npm package (already a declared dep in BOTH ui workspaces,
`^11.15.0`), lazy-imported so it stays out of the main chat bundle.

## Items

- **ITEM-1**: New `MermaidBlock` component in `src/components/common/` that mirrors
  the sibling `MarkdownTable.tsx` — a `[data-streamdown="mermaid-block"]` card
  (`my-4 flex flex-col gap-2 rounded-xl border border-border bg-sidebar p-2`) with a
  header toolbar row and a `rounded-md border border-border bg-background` body.
  Receives Streamdown's `CustomRendererProps` (`code`, `isIncomplete`, `language`,
  `meta`) plus an optional gallery-only `defaultMode` prop.
- **ITEM-2**: Source⇄render toggle — the kit `Segmented` control
  (`data-testid="mermaid-source-toggle"`, options `Diagram`/`Source`), local state
  defaulting to `render`; the body shows the rendered diagram in `render` mode and
  the raw source in `source` mode.
- **ITEM-3**: Mermaid diagram rendering — lazy-import `mermaid`, initialize with
  `securityLevel: 'strict'` and a theme synced to the app's light/dark
  (`useThemeOptional().isDark`), async-render `code` to an SVG string, DEFER while
  `isIncomplete` (streaming) showing a placeholder, and show an inline error state
  (source still reachable via the toggle) on parse failure. Stale async results are
  discarded via a cancellation guard.
- **ITEM-4**: Copy-source affordance — a ghost icon `Button`
  (`data-testid="mermaid-copy-source-btn"`) that writes `code` to the clipboard and
  toasts via `message.success`/`message.error` (the `MessageActions` idiom).
- **ITEM-5**: Download-SVG affordance — a ghost icon `Button`
  (`data-testid="mermaid-download-svg-btn"`) that downloads the rendered SVG as an
  `.svg` file (Blob + object-URL, the export-extension idiom); disabled until a
  successful render has produced SVG.
- **ITEM-6**: Register the mermaid custom renderer — a shared plugin-config module
  `src/modules/chat/core/utils/mermaidRenderers.ts` exporting a `PluginConfig`
  `{ renderers: [{ language: 'mermaid', component: MermaidBlock }] }`, wired via the
  `plugins` prop on BOTH chat `Streamdown` renderers (`extensions/text/components/
  TextContent.tsx` + `components/TextContent.tsx`) so the toggle applies on every
  chat render path.
- **ITEM-7**: Gallery story `mermaid.story.tsx` (registered in `stories/index.ts`)
  that renders `MermaidBlock` across its states — `render` mode, `source` mode
  (`defaultMode="source"`), an invalid-diagram error, and a streaming
  (`isIncomplete`) placeholder — so the visual/runtime gate exercises BOTH modes.
- **ITEM-8**: Flip the affordance detector from tracking to guarding — remove the
  `mermaid-toggle` entry from `scripts/affordance-audit-allowlist.json` so the
  `mermaid-toggle` rule now asserts `[data-streamdown="mermaid-block"] ⊃
  [data-testid="mermaid-source-toggle"]` is present (fails if the toggle regresses).

## Files to touch

- `src-app/ui/src/components/common/MermaidBlock.tsx` — NEW (ITEM-1..5)
- `src-app/ui/src/modules/chat/core/utils/mermaidRenderers.ts` — NEW (ITEM-6)
- `src-app/ui/src/modules/chat/extensions/text/components/TextContent.tsx` — EDIT, add `plugins` (ITEM-6)
- `src-app/ui/src/modules/chat/components/TextContent.tsx` — EDIT, add `plugins` (ITEM-6)
- `src-app/ui/src/dev/gallery/stories/mermaid.story.tsx` — NEW (ITEM-7)
- `src-app/ui/src/dev/gallery/stories/index.ts` — EDIT, register story (ITEM-7)
- `src-app/ui/scripts/affordance-audit-allowlist.json` — EDIT, remove `mermaid-toggle` (ITEM-8)
- `src-app/ui/tests/e2e/06-chat/mermaid-toggle.spec.ts` — NEW e2e (tests)
- `src-app/ui/src/components/common/MermaidBlock.test.tsx` — NEW unit (tests)

No backend change; no migration; no OpenAPI/types regen. Desktop UI resolves
`@/*` → `../../ui/src`, so the shared component + wiring apply to desktop with no
duplicated files.

## Patterns to follow

- **Streamdown-override component** — `src/components/common/MarkdownTable.tsx` is
  the exact sibling: a `components/common` component that replaces a Streamdown
  built-in, using the card chrome (bordered `bg-sidebar` container, an
  always-visible header bar carrying a `data-streamdown` marker + a right-aligned
  `flex items-center gap-0.5` toolbar of ghost icon `Button`s with `data-testid`s,
  above a `bg-background` body). Mirror its structure, tokens, and testid style.
- **Toggle control** — the kit `Segmented` (`src/components/ui/kit/segmented.tsx`),
  the app's standard segmented control (Tabs-without-panels); required `data-testid`.
- **Button variant policy** — kit `Button` "Spec B": peer icon-only buttons in one
  toolbar cluster share ONE variant → `ghost`, `size="icon"`, mandatory `tooltip`.
- **Copy-to-clipboard + toast** — `src/modules/chat/components/MessageActions.tsx`
  (`navigator.clipboard.writeText` → `message.success('Copied!')`).
- **Blob download** — `src/modules/chat/extensions/export/extension.tsx`
  (`new Blob(...)` → `URL.createObjectURL` → `<a download>` → `revokeObjectURL`).
- **Theme detection** — `useThemeOptional()` from `src/components/ui/kit/theme.tsx`
  (`.isDark`; the Optional variant tolerates rendering outside a ThemeProvider, e.g.
  a standalone gallery case).
- **Lazy heavy dep** — mirror Streamdown's own `lazy(() => import('mermaid...'))`
  by dynamic-importing `mermaid` inside the render effect (keeps it off the main
  bundle; loaded only when a mermaid block actually renders).
- **Gallery story** — `src/dev/gallery/stories/*.story.tsx` + `story.tsx`
  (`GalleryStory` with `cases[]`), registered in `stories/index.ts`.
