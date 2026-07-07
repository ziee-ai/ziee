# DECISIONS — pdf-viewer-architecture

All inputs the implementation needs, resolved up front so phase 5 runs nonstop.
DEC-1 is the significant architecture call; a human acknowledgment is requested
at the end of this phase before implementation begins (a process checkpoint, not
an unresolved item — the resolution below is the recommended answer).

### DEC-1: Backend page-on-demand rendering (A) vs client-side PDF.js (B)?
**Resolution:** (B) client-side PDF.js for real PDFs — ship the original PDF bytes to the browser and render canvas-per-page on demand with a per-page text layer.
**Basis:** codebase — the requested scope (text search with highlight + next/prev, text **selection/copy**, crisp zoom/fit) fundamentally needs a client text layer with per-glyph geometry. Option A renders flat server-side images (`processing/pdf.rs` → `<img>`), which have **no text layer**: selection/copy is impossible and search-highlight would require shipping char-bounding-boxes and hand-building an overlay — i.e. re-implementing ~60% of PDF.js, worse-tested. Option B delivers all of it natively (it is exactly what Firefox ships), removes the 50-page cap by rendering any page on demand from the full bytes, needs only one thin read-only backend endpoint, and stays lazy-chunked so the main bundle is unaffected. Trade-off accepted: `pdfjs-dist` is a large dep (mitigated by dynamic `import()`), and the original bytes reach the browser (already true via `download`; gated identically by `FilesPreview`).

### DEC-2: Do we change the backend ingest pipeline (stop rasterizing PDFs / lower the cap server-side)?
**Resolution:** No — leave `processing/pdf.rs` + `PREVIEW_PAGE_CAP` untouched. Real PDFs keep producing preview images + a thumbnail at ingest (now unused by the new viewer, harmless).
**Basis:** convention — minimize breakage. `preview_page_count`/`has_thumbnail`/thumbnails still feed file cards, the `File.store` auto-load gate, and truncation detection for non-PDF paged formats; touching ingest risks those with no viewer benefit. The 50-page *viewing* cap disappears anyway because the PDF viewer no longer consumes the capped images. (Dropping PDF pre-rasterization at ingest is a documented follow-up, out of scope here.)

### DEC-3: How do office documents (DOCX/DOC/RTF/ODT) render after this change?
**Resolution:** Unchanged — they keep the existing server-rendered image path (`PdfBody` in `body.tsx`, the Document module entry). Only the `application/pdf` entry moves to PDF.js.
**Basis:** codebase — office originals are `.docx`/`.rtf`/etc, NOT PDFs; the backend converts them to PDF→images and stores only the images, so no client-side PDF exists for PDF.js to load. The clean split is real-PDF → PDF.js, office → image path.

### DEC-4: How is `pdfjs-dist` added across the two UI workspaces?
**Resolution:** Add the same pinned `pdfjs-dist` version to `src-app/ui` and `src-app/desktop/ui`, update the root lockfile, keep `npx syncpack lint` clean, and import it via dynamic `import()` only inside `viewers/pdf/pdfjs.ts`.
**Basis:** convention — `.claude/FRONTEND_DEPS.md` + `.syncpackrc.json` require equal shared-dep versions in both workspaces and a green `npm run check`.

### DEC-5: `getDocument({ data })` detaches its input ArrayBuffer — how is that handled?
**Resolution:** Fetch the raw bytes as a Blob via `ApiClient.File.getRaw`, then pass a fresh `new Uint8Array(await blob.arrayBuffer())` copy to `getDocument`, so the underlying buffer PDF.js transfers to its worker is not one we reuse.
**Basis:** convention — PDF.js API contract (it neuters the passed buffer).

### DEC-6: Where does viewer view-state (zoom, current page, search) live?
**Resolution:** Component/hook-local React state (`usePdfDocument` + local `useState`/`useReducer`), NOT the global `Stores.File`.
**Basis:** codebase — the viewer lives in an ephemeral `FilePreviewDrawer`; per-file/per-open transient state must not leak into the shared file store (matches how the current `body.tsx` keeps only durable page-URL caches in the store).

### DEC-7: What permission + disposition does the raw-bytes endpoint use?
**Resolution:** `RequirePermissions<(FilesPreview,)>` + `Content-Disposition: inline` + `Cache-Control: FILE_CONTENT_CACHE_CONTROL`.
**Basis:** codebase — mirrors `get_preview` (the perm that already gates *viewing* a file), NOT `download_file`'s `FilesDownload` + `attachment`; a user who can preview must be able to render the PDF.

### DEC-8: How is the PDF.js worker loaded under Vite in both apps?
**Resolution:** Resolve the worker via `new Worker(new URL('pdfjs-dist/build/pdf.worker.min.mjs', import.meta.url), { type: 'module' })` and set it as `GlobalWorkerOptions.workerPort` inside `pdfjs.ts` (initialized once).
**Basis:** codebase — no CSP blocks workers/blobs (`tauri.conf.json` `csp: null`; no server-side `Content-Security-Policy`), and `import.meta.url` worker resolution is Vite-native, so no per-app vite.config change is required unless the build surfaces a resolution error.

### DEC-9: Zoom step scale, bounds, and default fit mode?
**Resolution:** Default = fit-width. Discrete zoom steps `[0.5, 0.75, 1.0, 1.25, 1.5, 2.0, 3.0]`, clamped to `[0.25, 4.0]`; buttons for zoom-in/out, fit-width, fit-page, actual-size (100%).
**Basis:** convention — standard PDF-viewer defaults; encoded in the pure `zoom.ts` helper so it is unit-testable and centrally tweakable.

### DEC-10: Search scope and text extraction?
**Resolution:** Whole-document find across all pages via PDF.js `page.getTextContent()`, extracted lazily and cached per page; matches enumerated in document order with wrap-around next/prev.
**Basis:** convention — the scope explicitly requires cross-document find + next/prev; per-page joined text with a char→item index map (in `search.ts`) drives highlight placement in the text layer.
