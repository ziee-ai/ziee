# Chunk `sdk-shell` — the reusable UI SHELL + permission primitives (CUT manifest)

Closes SDK_GAPS **FE-7** (no reusable shell — a 2nd app copies ~40 shell files),
**FE-8** (no shell↔domain-widget boundary), and the **ziee-permission** discussion
(the gating primitives are framework-level, used everywhere, not just the shell).

Two destinations:
- **`@ziee/framework/permissions`** (NEW subpath) — the 7 permission-gating
  primitives (`Can` / `usePermission` / `hasPermissionNow` / `hasPermission` /
  `evaluatePermission` / `types` / `index`). Framework-level because they gate
  every surface, not only the shell; `@ziee/shell` *consumes* them.
- **`sdk/packages/shell` (`@ziee/shell`, NEW package)** — the generic, slot-driven
  shell infra: `ThemeProvider` (+ theme tokens/utils), `AppErrorBoundary`, the
  app-bootstrap body (`AppShell`), the universal `LazyComponentRenderer` +
  `Loading`, `DivScrollY`, and the chromeless `BlankLayout`. Mirrors the
  `sdk/packages/{kit,framework,gallery}` src-export convention.

ziee becomes a **thin consumer**: `App.tsx` is now `loadModules()` +
`<AppShell authStore={useAuthStore}/>` (2 app-specific lines); every moved file
keeps a byte-thin re-export **shim** at its old `@/` path so ziee's ~hundreds of
importers compile unchanged, and the glob-based module discovery + the desktop
`localOverridePlugin` are both untouched.

## 3-BUCKET MAP

### BUCKET A — SHELL INFRA → `@ziee/shell` (generic, slot-driven, moved)
| ziee source (→ shim at same `@/` path) | → package dest |
|---|---|
| `components/ThemeProvider/ThemeProvider.tsx` | `src/theme/ThemeProvider.tsx` (reads `Stores.ConfigClient` via typed seam) |
| `components/ThemeProvider/accentPresets.ts` | `src/theme/accentPresets.ts` (verbatim) |
| `components/ThemeProvider/resolveTheme.ts` | `src/theme/resolveTheme.ts` (verbatim) |
| `components/ThemeProvider/themeColor.ts` | `src/theme/themeColor.ts` (`@/hooks/useTheme`→`./useTheme`) |
| `hooks/useTheme.ts` | `src/theme/useTheme.ts` (`ThemePreference` defined locally, not from config-client) |
| `components/AppErrorBoundary.tsx` | `src/error/AppErrorBoundary.tsx` (verbatim) |
| `core/components/Loading.tsx` | `src/components/Loading.tsx` (verbatim) |
| `core/components/LazyComponentRenderer.tsx` | `src/components/LazyComponentRenderer.tsx` (verbatim) |
| `components/common/DivScrollY.tsx` | `src/components/DivScrollY.tsx` (reads `Stores.AppLayout` via typed seam) |
| `modules/layouts/blank/BlankLayout.tsx` | `src/layouts/BlankLayout.tsx` (`@/…/themeColor`→`../theme/themeColor`) |
| `hooks/usePrefetchModules.ts` | `src/hooks/usePrefetchModules.ts` (reads `Stores.Routes` via typed seam) |
| `App.tsx` (bootstrap body) | `src/bootstrap/AppShell.tsx` (`authStore` prop; `loadModules` stays app-side) |

### BUCKET B — PERMISSION PRIMITIVES → `@ziee/framework/permissions` (moved)
`core/permissions/{Can.tsx, usePermission.ts, hasPermissionNow.ts,
hasPermission.ts, evaluatePermission.ts, types.ts, index.ts}` →
`sdk/packages/framework/src/permissions/*` (+ NEW `authView.ts` seam). Each old
`@/core/permissions/*` path becomes a shim (deep imports
`@/core/permissions/{Can,usePermission}` exist → all 7 shimmed).

### BUCKET C — STAYS app-side (domain / desktop-entangled)
- **Domain pages + per-module settings sections** — every `modules/*/…` page and
  `settings*Pages`-slot contributor. Unchanged.
- **FE-8 heavy domain widgets (severed, NEVER moved):** `components/kit/editor/*`
  (Plate), `components/common/{MarkdownCodeBlock,MermaidBlock,MarkdownTable,
  streamdownPlugins,markdownHeadings}` (streamdown/shiki/mermaid), `modules/file/
  viewers/pdf/*` (pdfjs), `modules/chat/core/utils/*` (streamdown), etc. →
  BUCKET-C-1 below.
- **app-layout + settings SHELL scaffold** (`modules/layouts/app-layout/**`,
  `modules/settings/**`) — DEFERRED (desktop-override entanglement) → see B-1 in
  BOUNDARY.md. `blank` layout (desktop-variant-free) DID move.
- **Router** — already extracted as `@ziee/framework/router` (optional subpath,
  with a `RoutePermissionGate` DI seam). NOT re-cut here; ziee's local
  `modules/router` is untouched (its migration to `@ziee/framework/router` is a
  separate chunk).

## FE-8 CLEAN BOUNDARY (severance PROVEN)

`@ziee/shell/src` has **zero** imports of platejs/streamdown/codemirror/shiki/
mermaid/pdfjs (grep-confirmed). The shell's ENTIRE non-relative dependency
surface is: `react`, `react-use`, `overlayscrollbars-react`, `@ziee/framework/*`,
`@ziee/kit` — and `@ziee/kit`'s package.json carries none of the six heavy widget
deps either. The 9 heavy domain-widget components (BUCKET-C-1) live only under
`modules/*` + `components/{kit/editor,common}` and were verified to have NO
inbound edge from any BUCKET-A/B file — so the shell severed clean **without**
inverting any control (no injection needed to keep the heavy deps out).

### BUCKET-C-1 — the heavy domain widgets the shell does NOT pull
`components/kit/editor/{CanvasImageElement,KitCodeEditor,KitMarkdownEditor,
MarkdownToolbar}` (platejs/codemirror) · `components/common/{MarkdownCodeBlock,
MarkdownTable,MermaidBlock,streamdownPlugins,markdownHeadings}` (streamdown/shiki/
mermaid) · `modules/file/viewers/{pdf/*,markdown/*,shared/RawCodeView}` (pdfjs/
codemirror) · `modules/chat/core/utils/{LazyStreamdown,HtmlBlock,…}` (streamdown).

## NO Rust / OpenAPI / generated-`types.ts` impact
Pure frontend. `git status` shows zero changes to any `api-client/types.ts`,
`openapi.json`, or Rust file (verified). E8 trivially byte-identical.
