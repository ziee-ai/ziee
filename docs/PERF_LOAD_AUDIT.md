# Ziee — Web/UI Load-Performance Audit

**Branch:** `perf/load-investigation` · **Date:** 2026-07-06 · **Scope:** initial
page-load cost of the React/Vite SPA (`src-app/ui`, shared into the shipped
`src-app/desktop/ui` bundle).

Everything below is **measured**, not estimated. Numbers come from a production
`vite build` (Vite 8 / Rolldown) plus a `rollup-plugin-visualizer` raw-data pass
to attribute bytes to modules. gzip/brotli are computed with Node `zlib` at max
level.

---

## 1. Measured baseline (origin/main)

### Initial critical path (what the browser must fetch before first paint)

`index.html` pulls exactly **one JS chunk + one CSS file** (`<script type=module src=index.js>` + `index.css`):

| Asset | raw | gzip | brotli |
|---|---:|---:|---:|
| `index-*.js` (entry) | 2,121 KB | **620 KB** | 503 KB |
| `index-*.css` | 212 KB | 31 KB | 25 KB |
| `index.html` | 2.4 KB | 1.2 KB | 0.9 KB |
| **Initial total** | **2.28 MB** | **653 KB** | **529 KB** |

### Whole app

- **507 JS chunks, 15.0 MB raw total** (all lazy chunks included).
- Route/page components **are** code-split (349 dynamic `import()` sites; each
  route element is a lazy `lazyWithPreload`).
- The problem is **not** missing route splitting — it is that the **entry chunk
  itself is 2.1 MB** because a large slice of the app is transitively reachable
  from the eagerly-loaded module graph (see §3).

### What's inside the 2.1 MB entry chunk (visualizer, rendered bytes ≈ 1.9× final)

| Module group | rendered KB | Notes |
|---|---:|---|
| `@base-ui/react` | 628 | UI primitive foundation. Imported per-subpath (good hygiene); large because broadly used. |
| `react-dom` | 448 | Unavoidable core. |
| `src/components` (the kit) | 273 | Shared component kit. |
| **`parse5`** | **190** | HTML parser — pulled in **only** by the markdown renderer. ✅ evicted by the quick win. |
| **`streamdown`** | **140** | Markdown renderer. ✅ evicted. |
| `src/modules/mcp` | 128 | |
| **`zod`** | 122 | Schema validation (forms / api-client). |
| `src/modules/file` | 118 | |
| `src/modules/chat` | 110 | |
| `react-router` | 92 | |
| **`react-day-picker`** | 89 | Date picker — needed on very few surfaces. |
| `react-hook-form` | 82 | |
| **`@shikijs/*` + `shiki`** | ~185 | Syntax highlighting — pulled in **only** by the markdown renderer. ✅ evicted. |
| **`micromark` + `mdast-*` + `rehype`** | ~120 | Markdown parse pipeline. ✅ evicted. |

### Largest **lazy** chunks (already off the critical path — good)

`emacs-lisp` 762 KB · `cpp` 611 KB · `wasm` (shiki oniguruma) 608 KB ·
`mermaid-parser.core` 589 KB · `cytoscape` 424 KB · `xlsx` 331 KB · `katex` 258 KB.
Plus **~130 per-language Shiki chunks** (one per grammar). These load on demand
(mermaid renders, spreadsheet export, math, a specific code language), so they do
**not** hit initial load — but see §4.4 for why they still cost bandwidth today.

### How the bundle is served

The SPA is served **only** by the desktop Tauri backend
(`ziee-desktop::serve_embedded_files`, a `rust-embed` fallback handler). The
Dockerized `ziee` server binary is **API-only** (port 9000, `/api/*`) and serves
no static assets. `serve_embedded_files` currently sets **no compression and no
`Cache-Control`** headers. Per direction from the maintainer, compression is a
**final optimization, not the solution** — the real lever is the bundle itself
(§3–4). Noted in §4.6 for completeness.

---

## 2. Top opportunities, ranked by (impact on initial load × effort)

| # | Opportunity | Est. initial-load impact | Effort | Status |
|---|---|---|---|---|
| 1 | **Lazy-load the Streamdown markdown stack** (streamdown + shiki + micromark/mdast/rehype + parse5) out of the entry chunk | **−125 KB gzip (−20% entry)** | Low | ✅ **DONE (this branch)** |
| 2 | **Stop eager-globbing `module.tsx`** / keep module registration lightweight so heavy render trees aren't dragged into entry (root cause) | High (potentially another −100–200 KB gzip) | High | 📋 Bigger play |
| 3 | **Make `usePrefetchModules` selective** — it currently prefetches **every** route chunk on idle, i.e. downloads the whole 15 MB app shortly after load | Bandwidth/CPU after load (not first paint) | Low–Med | 📋 Bigger play |
| 4 | **Trim Shiki bundled languages** (~130 grammars shipped) to a curated set | −MBs of lazy chunks; faster first code-block render | Med | 📋 Bigger play |
| 5 | **Lazy-load `react-day-picker`** (89 KB) — off the critical path for almost every user | −~25–30 KB gzip entry | Low | 📋 Targeted follow-up |
| 6 | **Investigate `zod` in entry** (122 KB rendered) — can validation be deferred to the surfaces that need it? | −~30 KB gzip entry | Med | 📋 Investigate |
| 7 | **Drop the dead `GalleryPage` chunk from prod** — the route is `import.meta.env.DEV`-gated but the `import()` call still emits an 83 KB chunk | −83 KB disk (not load) | Low | 📋 Trivial |
| 8 | **Audit `@base-ui/react` usage** (628 KB) for unused component subpaths | Low ROI (foundational) | Med | 📋 Low priority |
| 9 | **Route-level split is healthy** — preserve it; the wins above are about what leaks *into* entry, not the routes | — | — | ✅ Keep |
| 10 | **Compression + immutable cache headers** on `serve_embedded_files` | −~15% wire + instant repeat visits | Low | 📋 "Final optimization" (deprioritized per maintainer) |

---

## 3. Root cause: why the whole app lands in the entry chunk

Two mechanisms pull far more than a shell into the first chunk:

1. **`loadModules()` uses `import.meta.glob('./**/module.tsx', { eager: true })`**
   (`src/modules/loader.ts`). Every module's `module.tsx` — and anything it
   *statically* imports (stores, widgets, slot components, chat extensions) — is
   in the entry chunk. Route **page** elements are lazy, but the registration
   graph is not.

2. **Static import chains from the eager graph into heavy leaf libs.** The
   markdown renderer is the clearest example and the subject of the quick win
   below.

Fixing #1 structurally (lazy module registration, or splitting registration
metadata from render code) is the single biggest remaining lever, but it is an
architectural change → **feature-lifecycle candidate**, not a mechanical quick
win.

---

## 4. Quick win implemented in this branch

### 4.1 What & why — lazy-load the markdown renderer

`streamdown` drags in Shiki (syntax highlighting), the micromark/mdast/rehype
markdown pipeline, and `parse5` (HTML parsing) — together **~450 KB rendered /
~125 KB gzip**. None of it is needed until a **rendered markdown surface**
mounts (an assistant chat message, a file/skill/workflow markdown view). It was
in the entry chunk purely because of a static import chain.

A first attempt — wrapping only the `Streamdown` **component** in `React.lazy` —
had **zero effect** (measured). The cause was a classic **ineffective dynamic
import**: `components/common/MarkdownTable.tsx` still carried a *static*
`import { TableCopyDropdown, TableDownloadDropdown } from 'streamdown'`, so the
whole package stayed in entry and the dynamic import just referenced the entry
copy. Only after making **every** runtime `streamdown` import dynamic did the
split take effect.

### 4.2 The change (contained, 3 concerns, 7 files)

- **New** `modules/chat/core/utils/LazyStreamdown.tsx` — a drop-in `Streamdown`
  wrapper: `React.lazy(() => import('streamdown'))` behind `<Suspense>`, with the
  **raw markdown text as the fallback** (pre-wrapped) so there is no blank flash
  and content degrades gracefully if the chunk fails.
- **5 call sites** swapped `import { Streamdown } from 'streamdown'` →
  `from '@/modules/chat/core/utils/LazyStreamdown'`: both chat `TextContent`,
  `skill/SkillDetailDrawer`, `workflow/StepOutputExpander`, `file markdown body`.
- **`MarkdownTable.tsx`** — its two Streamdown control imports made dynamic
  (`React.lazy` + `<Suspense>`). This was the **last static importer** and the
  actual unlock. Safe because `MarkdownTable` is a `components.table` renderer
  that only ever mounts *inside* an already-loaded Streamdown tree.
- Regenerated gallery artifacts (state-matrix / coverage) for the new surface.

### 4.3 Measured before → after

| | Entry JS raw | Entry JS gzip | Entry JS brotli | Initial gzip | Initial brotli |
|---|---:|---:|---:|---:|---:|
| **Before** | 2,121 KB | 620 KB | 503 KB | 653 KB | 529 KB |
| **After** | 1,699 KB | **495 KB** | 402 KB | **527 KB** | 427 KB |
| **Δ** | −422 KB | **−125 KB (−20%)** | −101 KB | −126 KB (−19%) | −102 KB |

The evicted stack moves into a lazy chunk fetched on first markdown render and
cached thereafter. Shipped desktop bundle confirms the same win: entry **491 KB
gzip** (from ~620 KB).

### 4.4 Gates (all green)

- `vite build` ✓ · `tsc --noEmit` ✓ · `npm run check` (all 13 sub-checks) ✓
- Gallery render smoke ✓ — the gallery renders, `[data-streamdown]` elements are
  present (lazy chunk resolves + markdown renders), **0 console errors**.

---

## 5. Bigger-plays backlog (plan only — feature-lifecycle candidates)

1. **De-eager the module registry (§3, #2).** Split each module's *registration
   metadata* (routes, slots, order) from its *render code* so `loadModules` can
   register without statically importing heavy component trees. Biggest structural
   lever; touches every `module.tsx`.
2. **Selective prefetch (§2, #3).** `usePrefetchModules` calls every route's
   preloader on `requestIdleCallback`, i.e. downloads the entire 15 MB app after
   first paint. Replace with route-affinity / hover-intent prefetch, or cap it.
3. **Curate Shiki languages (§2, #4).** ~130 grammars ship as lazy chunks (and
   the eager prefetch above pulls many). Restrict Streamdown/Shiki to a common
   language set; lazy-register the long tail.
4. **Targeted leaf lazy-loads:** `react-day-picker` (#5), and an investigation of
   `zod`'s entry-chunk footprint (#6).
5. **Compression + immutable caching** on `serve_embedded_files` (#10) — the
   "final optimization" once the bundle work above is done: brotli takes the
   current initial transfer to ~427 KB, and `Cache-Control: immutable` on hashed
   assets makes repeat visits near-instant.

---

## 6. How to reproduce the measurements

```bash
cd src-app/ui
npx vite build                     # chunk sizes printed by Rolldown
# entry gzip/brotli:
node -e 'const fs=require("fs"),z=require("zlib");const d="../dist/ui/assets";\
const j=fs.readFileSync(d+"/"+fs.readdirSync(d).find(f=>/^index-.*\.js$/.test(f)));\
console.log("gzip",(z.gzipSync(j,{level:9}).length/1024|0)+"KB","brotli",(z.brotliCompressSync(j).length/1024|0)+"KB")'
```

Per-module attribution used `rollup-plugin-visualizer` (`template:'raw-data'`)
temporarily added to `vite.config.ts`; not committed.
