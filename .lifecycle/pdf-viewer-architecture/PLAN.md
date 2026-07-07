# PLAN — pdf-viewer-architecture (F4)

## Context

Today the PDF viewer (`ui/src/modules/file/viewers/pdf/body.tsx`) renders a
stack of **pre-rasterized `<img>` pages** produced at ingest by the backend
(`server/src/modules/file/processing/pdf.rs` → `generate_images`), capped at
`PREVIEW_PAGE_CAP = 50` (`processing/mod.rs`). Consequences (from
`CAPABILITY_AUDITED.md` cluster 1b): 50-page truncation banner, no real page
navigation, **no zoom**, **no text search**, **no text selection/copy** —
because the client only ever sees flat images with no text layer.

The chosen architecture (see DECISIONS DEC-1) is **(B) client-side PDF.js** for
real PDFs: ship the original PDF bytes to the browser and render
canvas-per-page on demand with a text layer, giving native zoom, find, and
selection. Office documents (DOCX/RTF/ODT), which the backend converts to
PDF→images and for which **no client-side PDF exists**, keep the existing
image-page renderer unchanged.

## Items

- **ITEM-1**: Add backend handler `get_raw` serving a file's head-version **original bytes inline** (`Content-Type` from stored mime, `Content-Disposition: inline`, `Cache-Control: private, …`), gated by `FilesPreview`, owner-scoped (cross-user → 404). This is the byte source the client PDF.js renderer loads real PDFs from — distinct from `download` (which is `FilesDownload`-gated + `attachment` disposition).
- **ITEM-2**: Register `GET /files/{file_id}/raw` in `routes.rs` (BEFORE the `/files/{file_id}` catch-all, alongside `/preview`), with aide OpenAPI docs; regenerate `openapi.json` + `api-client/types.ts` for **both** binaries (server UI + desktop) via `just openapi-regen`, yielding a generated `ApiClient.File.getRaw`.
- **ITEM-3**: Add `pdfjs-dist` as a dependency to **both** UI workspaces (`src-app/ui` + `src-app/desktop/ui`) at a single pinned version (syncpack-clean, root `overrides` if needed); wire the PDF.js **worker** so it loads under Vite in both apps; ensure `pdfjs-dist` is imported via **dynamic `import()`** so it lands in a lazy chunk and never bloats the main bundle.
- **ITEM-4**: Add a PDF.js loader/util module (`viewers/pdf/pdfjs.ts`) that dynamic-imports pdfjs, configures the worker once, and a `usePdfDocument` hook (`viewers/pdf/usePdfDocument.ts`) that fetches raw bytes via `ApiClient.File.getRaw`, opens the doc (`getDocument({ data })`), exposes `{ status, numPages, getPage, error }`, and **destroys** the loading task + document + aborts fetch on unmount (no leaks).
- **ITEM-5**: New `PdfJsBody` component (`viewers/pdf/pdfjs-body.tsx`) that renders **canvas-per-page on demand** (IntersectionObserver windowing with reserved page height, mirroring the current body's approach) at the current zoom scale, each page wrapped with a **PDF.js text layer** overlay positioned over the canvas (enables selection + search highlight). Replaces the image body for the `application/pdf`/`pdf` entry ONLY.
- **ITEM-6**: Page navigation UI — a live **"Page N of M"** indicator that tracks scroll position (updates as the user scrolls), **prev/next** buttons, and a **jump-to-page** input (Enter/change scrolls the matching page into view). Pure page↔scroll mapping logic factored into a testable helper.
- **ITEM-7**: Zoom controls — **zoom in / zoom out** (discrete scale steps), **fit-width**, **fit-page**, **actual size (100%)**; changing zoom re-renders visible canvases at the new scale. Fit-width/fit-page scale computation factored into a pure, unit-testable helper (`viewers/pdf/zoom.ts`).
- **ITEM-8**: Text search — a **find box** (toggled from the toolbar / Ctrl-F within the viewer), searching PDF.js `getTextContent()` across pages, **highlighting** matches in the text layer, **next/prev** match navigation with an **"x of N"** count, and scroll-to-match. Match enumeration/ordering + current-match cycling factored into a pure, unit-testable helper (`viewers/pdf/search.ts`).
- **ITEM-9**: Text **selection + copy** — the per-page text layer makes native selection work across the rendered page; verify selecting text and copying yields the underlying text (no image-only dead zone).
- **ITEM-10**: Split `viewers/pdf/module.tsx` so the **PDF entry** (`application/pdf`, ext `pdf`) uses `PdfJsBody` + a new `PdfJsHeader` (hosting page-nav + zoom + find + download toolbar), while the **Document entry** (DOCX/DOC/RTF/ODT/…) keeps the existing image `PdfBody` + `PdfHeader` unchanged (add a clarifying comment that `PdfBody` is now the office/image path). Real PDFs no longer show the 50-page truncation banner.
- **ITEM-11**: Gallery coverage + offline fixture — embed a tiny deterministic PDF (base64) as a fixture; extend the gallery `mockApi` with a **binary-response** capability (today it only ever returns `jsonResponse`) and register a cassette route for `GET /files/{id}/raw` returning the fixture PDF bytes as `application/pdf`, so `PdfJsBody` renders a real page in the backend-free gallery; add gallery cells for the new conditional states (**loading / loaded / error / find-open**) so `check:state-matrix` passes.

## Files to touch

### Backend (`src-app/server`)
- `src/modules/file/handlers/management.rs` — new `get_raw` handler (+ its `*_docs` aide fn) [ITEM-1]
- `src/modules/file/handlers/mod.rs` — re-export `get_raw` if handlers are re-exported there [ITEM-1]
- `src/modules/file/routes.rs` — register `/files/{file_id}/raw` [ITEM-2]

### Generated (mechanical — excluded from UI-gate / coverage law)
- `src-app/ui/openapi/openapi.json`, `src-app/ui/src/api-client/types.ts` [ITEM-2]
- `src-app/desktop/ui/openapi/openapi.json`, `src-app/desktop/ui/src/api-client/types.ts` [ITEM-2]

### Frontend (`src-app/ui`, shared into desktop via the `@/` alias plugin)
- `package.json` (ui) + `src-app/desktop/ui/package.json` + root `package.json`/`.syncpackrc.json`/`package-lock.json` — `pdfjs-dist` dep [ITEM-3]
- `src-app/ui/vite.config.ts` + `src-app/desktop/ui/vite.config.ts` — PDF.js worker wiring (only if `import.meta.url` worker resolution needs config) [ITEM-3]
- `src/modules/file/viewers/pdf/pdfjs.ts` (new — loader/worker) [ITEM-4]
- `src/modules/file/viewers/pdf/usePdfDocument.ts` (new — doc lifecycle hook) [ITEM-4]
- `src/modules/file/viewers/pdf/pdfjs-body.tsx` (new — canvas + text-layer renderer) [ITEM-5, ITEM-9]
- `src/modules/file/viewers/pdf/pdfjs-header.tsx` (new — toolbar) [ITEM-6, ITEM-7, ITEM-8]
- `src/modules/file/viewers/pdf/zoom.ts` (new — pure fit/scale helper) [ITEM-7]
- `src/modules/file/viewers/pdf/search.ts` (new — pure match helper) [ITEM-8]
- `src/modules/file/viewers/pdf/nav.ts` (new — pure page↔scroll helper) [ITEM-6]
- `src/modules/file/viewers/pdf/module.tsx` — split PDF vs Document entries [ITEM-10]
- `src/modules/file/viewers/pdf/body.tsx` + `header.tsx` — keep for office; add clarifying comment [ITEM-10]
- `src/modules/file/viewers/pdf/pdf-fixture.ts` (new — base64 tiny PDF for gallery/tests) [ITEM-11]
- `src/dev/gallery/mockApi.ts` — add binary-response support + `/files/{id}/raw` cassette route [ITEM-11]
- `src/dev/gallery/overlays.tsx` (+ any state/coverage registry) — PDF viewer gallery states [ITEM-11]

## Patterns to follow

- **Backend handler + route + docs**: mirror `get_preview` / `get_thumbnail` in `handlers/management.rs` and their registration in `routes.rs` exactly (same `RequirePermissions<(FilesPreview,)>`, same `get_by_id_and_user` ownership check → 404, same header/cache pattern, same `*_docs` aide fn shape). [ITEM-1, ITEM-2]
- **On-demand page windowing**: mirror the existing `body.tsx` IntersectionObserver + reserved-height + OverlayScrollbars-viewport-as-root approach for lazy canvas rendering. [ITEM-5]
- **Viewer module shape**: follow the existing `viewers/pdf/module.tsx` `FileViewerModule[]` declaration and the `viewers/image/*` / `viewers/markdown/*` body+header split. [ITEM-10]
- **Store/data access**: use `Stores.File` + `ApiClient.File.*` (blob response) as `body.tsx` does today; keep view-scoped state (zoom, current page, search) **local to the component/hook** (the viewer is an ephemeral drawer), not in a global store. [ITEM-4..8]
- **Gallery fixture/cassette**: mirror the existing `overlay-file-preview-drawer` entry (`overlays.tsx`) + the gallery mock-API cassette pattern for the new `getRaw` call. [ITEM-11]
- **Dep hygiene**: follow `.claude/FRONTEND_DEPS.md` + `.syncpackrc.json` — same version in both workspaces, `npm run check` clean. [ITEM-3]
