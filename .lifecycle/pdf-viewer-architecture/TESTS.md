# TESTS — pdf-viewer-architecture

Every ITEM is covered by ≥1 TEST. Pure logic (zoom/search/nav helpers, mockApi
binary, module mapping) gets `unit`; backend endpoint gets `integration`; every
user-visible viewer flow gets an `e2e` spec. Mock only the external boundary —
the e2e specs drive the real renderer end-to-end (real PDF bytes → real pdfjs).

## Backend

- **TEST-1** (tier: integration) [covers: ITEM-1, ITEM-2] file: `src-app/server/tests/file/pdf_raw_test.rs` — asserts: upload `test_data/multipage.pdf`, `GET /files/{id}/raw` returns 200 with `Content-Type: application/pdf`, `Content-Disposition: inline`, and a body byte-identical to the uploaded PDF (route is registered ⇒ ITEM-2; handler serves inline original bytes ⇒ ITEM-1).
- **TEST-2** (tier: integration) [covers: ITEM-1] file: `src-app/server/tests/file/pdf_raw_test.rs` — asserts: user B (who can download their own files) requesting user A's file `GET /files/{id}/raw` gets 404 (owner-scoped — the bytes never leak). The `files::download` gate 403 is the standard `RequirePermissions` extractor (as on `download_file`) and isn't separately asserted because the harness's registered users all carry the default group's `files::download` (a download-less user isn't constructible) — documented in the test.

## Frontend — pure-logic unit (node --test)

- **TEST-3** (tier: unit) [covers: ITEM-7] file: `src-app/ui/src/modules/file/viewers/pdf/zoom.test.ts` — asserts: the discrete zoom-step ladder — `nextZoomStep(current, +1)` returns the next-larger step, `nextZoomStep(current, -1)` the next-smaller, both clamped to `[0.25, 4.0]`; a scale between steps snaps to the correct neighbour; actual-size step = 1.0 is present.
- **TEST-5** (tier: unit) [covers: ITEM-6] file: `src-app/ui/src/modules/file/viewers/pdf/nav.test.ts` — asserts: `clampPage(n, numPages)` clamps to `[1, numPages]`, and `parseJump(input, numPages)` parses a user-typed page string (ignoring non-numeric / out-of-range → clamped or null), used by the jump-to-page input.
- **TEST-6** (tier: unit) [covers: ITEM-11] file: `src-app/ui/src/dev/gallery/mockApi-binary.test.ts` — asserts: `makeBinaryResponse(bytes, 'application/pdf')` (the pure helper backing the `/files/{id}/raw` cassette route) produces a `Response` with `application/pdf` content-type whose `arrayBuffer()` equals the fixture bytes (proves the new binary-response path returns bytes, not JSON). Standalone alias-/JSX-free module so `node --test` can load it.

## Frontend — e2e (Playwright, gallery)

> The e2e specs drive the REAL `PdfJsBody` (real pdfjs-dist PDFViewer + canvas +
> text layer + PDFFindController) through the backend-free gallery
> (`overlay-file-preview-drawer` surface); the gallery mock-API serves the
> deterministic sample PDF for `/files/{id}/raw`. Only the byte-fetch boundary is
> mocked — the raw endpoint itself is covered by the Rust integration tests
> (TEST-1/2). Run under `playwright.visual.config.ts`.

- **TEST-8** (tier: e2e) [covers: ITEM-3, ITEM-4, ITEM-5, ITEM-9, ITEM-10] file: `src-app/ui/tests/e2e/visual/pdf-viewer.spec.ts` — asserts: after uploading a multi-page PDF and opening its preview, pdfjs loads (worker chunk fetched, no console error), page 1 renders a `<canvas>` with a positioned text layer, the pdfjs toolbar (not the office image body) is shown, the selected page text is selectable/copyable, and the truncation banner is absent.
- **TEST-9** (tier: e2e) [covers: ITEM-6] file: `src-app/ui/tests/e2e/visual/pdf-viewer.spec.ts` — asserts: the indicator shows "Page 1 of N"; clicking next advances to page 2 (indicator + scroll), prev returns, and typing a page number in jump-to-page scrolls that page into view and updates the indicator.
- **TEST-10** (tier: e2e) [covers: ITEM-7] file: `src-app/ui/tests/e2e/visual/pdf-viewer.spec.ts` — asserts: zoom-in increases the rendered canvas pixel width, zoom-out decreases it, fit-width fills the viewport width, and actual-size returns to 100%.
- **TEST-11** (tier: e2e) [covers: ITEM-8] file: `src-app/ui/tests/e2e/visual/pdf-viewer.spec.ts` — asserts: opening find, typing a term present in the PDF drives `PDFFindController` to show an "x of N" count with visible highlight(s), next/prev move the active highlight and scroll to it, and a term not present shows "0" matches.
- **TEST-12** (tier: e2e) [covers: ITEM-11] file: `src-app/ui/tests/e2e/visual/pdf-viewer.spec.ts` — asserts: the gallery `overlay-file-preview-drawer` (PDF fixture) renders the loaded PDF page offline via the mockApi binary route with zero console errors / failed requests (backs the gallery loaded/find-open state cells that `check:state-matrix` + `gate:ui` enforce).

## Coverage map (every ITEM → ≥1 TEST)

- ITEM-1 → TEST-1, TEST-2
- ITEM-2 → TEST-1
- ITEM-3 → TEST-8
- ITEM-4 → TEST-8
- ITEM-5 → TEST-8
- ITEM-6 → TEST-5, TEST-9
- ITEM-7 → TEST-3, TEST-10
- ITEM-8 → TEST-11
- ITEM-9 → TEST-8
- ITEM-10 → TEST-8 (routing proven end-to-end; the JSX `module.tsx` can't be unit-loaded by `node --test`, which strips TS types but not JSX)
- ITEM-11 → TEST-6, TEST-12
