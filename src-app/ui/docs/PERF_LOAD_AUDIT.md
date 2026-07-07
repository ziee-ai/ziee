# Ziee — Web/UI Load-Performance Audit

**Branch:** `perf/load-investigation` · **Scope:** the initial page-load cost of
the React/Vite SPA (`src-app/ui`, shared into the shipped `src-app/desktop/ui`
bundle) and how the Axum layer serves it.

All numbers are **measured** from production `vite build` (Vite 8 / Rolldown).
Sizes are **raw (uncompressed) bytes as actually served** unless a column says
gzip/brotli. The baseline is the branch's fork point (`2ec05e6c`), built in the
same dependency environment, so before/after is apples-to-apples.

---

## 1. Measured baseline

The SPA entrypoint (`index.html`) statically pulls its first-paint assets via
`<script type=module>` + `<link rel=modulepreload>`. At baseline the whole
statically-reachable app is one entry chunk, so first paint is **2 files**:

| First-paint set (baseline) | raw | gzip | brotli |
|---|---:|---:|---:|
| entry `index-*.js` | 2,120 KB | — | — |
| `index-*.css` | 212 KB | — | — |
| **Total first paint** | **2,332 KB** | **651 KB** | 526 KB |

- Whole app: **~15 MB across ~500 JS chunks** (route/feature chunks are already
  lazy — 349 dynamic imports). The problem is not missing route-splitting; it's
  that **2.1 MB is glued into the first-paint entry chunk**.
- **Served with no compression and no `Cache-Control`** — so a first-time
  visitor downloaded the full **2,332 KB raw** over the wire, and re-downloaded
  it on every visit.

### What contributes to the entry chunk (visualizer, scaled to real bytes ±10%)

| Contributor | ~Real KB | |
|---|---:|---|
| `@base-ui/react` | ~309 | UI primitive foundation |
| `react-dom` | ~221 | core runtime |
| `src/components/ui` (kit) | ~126 | |
| **`streamdown` + Shiki + micromark/mdast/rehype + `parse5`** | **~250** | markdown renderer stack — **only needed when a message renders** |
| app modules (mcp/file/chat/llm-provider/layouts) | ~250 | |
| **`shiki` core** (via `RawCodeView`) | **~90** | code-file viewer — **only needed when a code file is viewed** |
| `zod` | ~60 | form/schema validation |
| `react-router` | ~45 | |
| `react-day-picker` | ~44 | date picker |
| `react-hook-form` | ~41 | |
| misc (overlayscrollbars, tailwind-merge, date-fns, sonner, lucide, virtual) | ~130 | |

The two **bold** groups (~340 KB raw combined) are renderers that don't belong in
first paint — the quick-wins below evict them.

### How it's served

The SPA is served **only** by the desktop Tauri backend
(`ziee-desktop::serve_embedded_files`, a `rust-embed` fallback). The Dockerized
`ziee` server binary is **API-only** and serves no static assets. This handler
had **no compression and no cache headers**. The local Tauri webview loads assets
over the `tauri://` protocol; this HTTP path serves the **Remote Access tunnel**
(phones/browsers), where compression + caching matter most.

---

## 2. Top opportunities, ranked by (impact on first paint × effort)

| # | Opportunity | First-paint impact | Effort | Status |
|---|---|---|---|---|
| 1 | **Lazy-load the Streamdown markdown stack** out of the entry chunk | **−422 KB raw / −125 KB gzip** | Low | ✅ **DONE** |
| 2 | **Lazy-load Shiki in `RawCodeView`** (code-file highlighter) | **−146 KB raw / −44 KB gzip** | Low | ✅ **DONE** |
| 3 | **br/gzip compression + immutable cache headers** on the Axum static serving | **wire −78%; repeat visits ≈ 0** | Low | ✅ **DONE** |
| 4 | **Chunk config** — evaluate a vendor split | ~0 (measured net-negative for first paint) | Low | ✅ Evaluated → not applied (see §4.4) |
| 5 | **De-eager the `import.meta.glob('module.tsx',{eager:true})` registry** (root cause: the whole app graph is first-paint) | High | High | 📋 Bigger play |
| 6 | **Make `usePrefetchModules` selective** — it prefetches **every** route on idle, pulling ~MBs after paint | Big on total session bytes | Low–Med | 📋 Bigger play |
| 7 | **Curate the ~77 Shiki grammar chunks** (5 MB of lazy chunks) | On-demand bytes | Med | 📋 Bigger play |
| 8 | **Lazy `react-day-picker` (~44 KB) / trim `zod` (~60 KB)** in entry | −~30–40 KB gzip | Low–Med | 📋 Targeted follow-up |

---

## 3. Root cause: why the whole app is in first paint

`loadModules()` uses `import.meta.glob('./**/module.tsx', { eager: true })`
(`src/modules/loader.ts`). Every module's `module.tsx` — and everything it
*statically* imports (stores, widgets, slot components, chat extensions) — is in
the first-paint graph. Route **page** elements are lazy, but the registration
graph is not, and it statically reaches heavy leaf libraries. The renderer
lazy-loads below cut the worst offenders; a structural fix (lazy module
registration) is the biggest remaining lever (#5).

---

## 4. Quick-wins implemented (this branch)

### 4.1 Lazy-load the Streamdown markdown renderer

`streamdown` drags in Shiki, the micromark/mdast/rehype pipeline, and `parse5`.
Routed every `Streamdown` render through a `React.lazy` boundary
(`LazyStreamdown.tsx`) with the raw markdown text as the Suspense fallback (no
blank flash). The unlock was making `MarkdownTable`'s two `streamdown` control
imports dynamic too — that static import was an *ineffective-dynamic-import* that
had kept the whole package in the entry chunk. The loaders go through core's
`lazyWithPreload`, so the desktop override **preloads the embedded chunk in the
Tauri webview** (no fallback flash) while web/tunnel builds stay deferred.

### 4.2 Lazy-load Shiki in `RawCodeView`

The raw-code file viewer statically imported `codeToHtml` + `bundledLanguages`
from `shiki`, keeping ~90 KB of Shiki core in first paint independent of
Streamdown. Moved both behind a cached `import('shiki')` inside the existing
async highlight effect (language validated against the lazily-loaded grammar
table; unknown ids fall back to plain text exactly as before).

### 4.3 Axum compression + immutable cache headers

`ziee-desktop::serve_embedded_files` now:
- sets `Cache-Control: public, max-age=31536000, immutable` on content-hashed
  `assets/*` files and `no-cache` on the (unhashed) HTML entry — repeat visits
  re-fetch **only** the HTML + whatever actually changed;
- is wrapped in a `tower-http` `CompressionLayer` (br + gzip) **scoped to the
  static fallback only** (API/SSE routes untouched; tower-http's DefaultPredicate
  also skips `text/event-stream`). Encoding is negotiated from `Accept-Encoding`.

Verified: `cargo check -p ziee-desktop` passes with the new
`compression-br`/`compression-gzip` tower-http features.

### 4.4 Chunk config — measured, not applied

Tested a Rolldown vendor split (`react-dom`/`base-ui` → stable chunks for
cross-deploy caching). Measured result: it **fragments first paint from 2 files
to 44 preloaded requests for +17 KB raw**, buying only a repeat-visit caching
benefit. Since first-load speed is the goal, Rolldown's default single-entry
chunking is better here, so the split was **not** applied. It remains a valid
*repeat-visit* optimization to pair with the immutable headers if returning-user
perf is later prioritized (a one-line `build.rolldownOptions` addition).

### 4.5 Measured before → after (first-paint set, raw as served)

| | first-paint files | raw | gzip | brotli |
|---|---:|---:|---:|---:|
| **Before** (baseline) | 2 | 2,332 KB | 651 KB | 526 KB |
| **After** (renderer lazy-loads) | 2 | **1,764 KB** | **480 KB** | 388 KB |
| **Δ** | — | **−568 KB (−24%)** | **−171 KB (−26%)** | **−138 KB (−26%)** |

On the wire, once compression (§4.3) is active, first load drops from the
**2,332 KB** raw baseline to **~388 KB brotli** (**−83%**), and repeat visits of
unchanged assets are served from cache (`immutable`). The desktop-shipped bundle
mirrors these numbers (shared source).

**Gates (all green):** `vite build`, `tsc`, `npm run check` (all sub-checks),
`cargo check -p ziee-desktop`, gallery render smoke (streamdown renders, 0
console errors).

---

## 5. Bigger-plays backlog (plan only — feature-lifecycle candidates)

1. **De-eager the module registry (§3, #5).** Split each module's registration
   metadata from its render code so `loadModules` registers without statically
   importing heavy component trees. Biggest remaining first-paint lever.
2. **Selective prefetch (#6).** `usePrefetchModules` fires every route loader on
   idle → downloads much of the ~15 MB app after first paint. Replace with
   hover-intent / route-affinity prefetch, or cap it.
3. **Curate Shiki grammars (#7).** ~77 language chunks (5 MB) ship lazily;
   restrict to a common set and lazy-register the long tail.
4. **Targeted leaf lazy-loads / trims (#8):** `react-day-picker`, `zod`.
5. **Vendor-chunk split for repeat visits (§4.4):** optional, pairs with the
   immutable cache headers already shipped.

---

## 6. Reproduce

```bash
cd src-app/ui && npx vite build
# first-paint bytes (sum of index.html's script + modulepreload assets):
node -e 'const fs=require("fs"),z=require("zlib");const h=fs.readFileSync("../dist/ui/index.html","utf8");\
const r=[...h.matchAll(/assets\/([^"]+\.(?:js|css))/g)].map(m=>m[1]);\
const a=Buffer.concat(r.map(x=>fs.readFileSync("../dist/ui/assets/"+x)));\
console.log(r.length,"files","raw",(a.length/1024|0)+"KB","gzip",(z.gzipSync(a,{level:9}).length/1024|0)+"KB","brotli",(z.brotliCompressSync(a).length/1024|0)+"KB")'
```
Per-module attribution used `rollup-plugin-visualizer` (raw-data) temporarily;
not committed.
