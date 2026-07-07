# FIX_ROUND-1 — merge ledger → fix all confirmed → re-audit

All 9 confirmed findings from the phase-6 blind audit (3 fork reviewers,
13 angles) were fixed. No finding was rejected as a false positive — every one
was a real (if mostly low-severity) defect. The 5 `cleared` ledger entries were
verified-clean angles, not defects.

## Fixes applied

- **[MED perms-authz] get_raw permission** (`management.rs`): re-gated from `FilesPreview` → **`FilesDownload`**, because the endpoint serves the EXACT original bytes (a download-equivalent capability). Rendered preview *images* stay on `FilesPreview` (`get_preview`). This honors an admin who withholds download from a group; the default Users group grants both (migration 27), so default users are unaffected. DEC-7 amended. Integration test `TEST-2` asserts the security-critical property — **cross-user → 404** (bytes never leak). Note: the 403 perm-gate path is NOT unit-assertable here because the test harness's `create_user_with_permissions` also assigns the **default group** (which grants `files::download`, migration 27), so a download-less user cannot be constructed; the gate is the same standard `RequirePermissions` extractor `download_file` already relies on (documented in the test).
- **[MED correctness] stepPage stale state** (`pdfjs-body.tsx`): prev/next now step from `controllerRef.current.getCurrentPage()` (the viewer's live page) instead of async React state, so rapid double-clicks advance correctly.
- **[MED resource-lifecycle] controller teardown** (`pdfjs.ts`): `destroy()` now calls `pdfViewer.cleanup()` + `pdfViewer.setDocument(null)` (in addition to `eventBus.off`), which empties the `.pdfViewer` element and drops `pagesCount` to 0 — making any lingering scroll/resize listener inert and preventing stacked viewers / orphaned canvases on document swap or StrictMode remount.
- **[LOW api-contract] undocumented 403** (`get_raw_docs`): added `.response_with::<403,…>()`; regenerated `openapi.json` for both binaries (verified the operation now lists 200/401/403/404).
- **[LOW a11y] find-count not announced** (`pdfjs-body.tsx`): added `aria-live="polite"` to the "x of N" match count.
- **[LOW a11y] Ctrl+F reachability** (`pdfjs-body.tsx`): the scroll container is now `tabIndex={0}` (focusable), so the in-viewer find shortcut fires reliably when the user has clicked into the document.
- **[LOW state-management] stale page indicator on swap** (`pdfjs-body.tsx`): on controller (re)creation the toolbar resets `currentPage`/`pageInput`/find state to the new document's baseline.
- **[LOW patterns-conformance] hand-rolled dividers** (`pdfjs-body.tsx`): replaced `<div class="h-5 w-px bg-border">` with the kit `<Separator orientation="vertical">`.
- **[LOW tests-quality] uncovered disabled logic** (`nav.ts` + `nav.test.ts`): extracted the toolbar boundary predicates into pure `canPrevPage`/`canNextPage` helpers, used by the JSX and covered by a new unit test.

## Verification after fixes

- `tsc` clean (both workspaces); `npm run check` green in **both** `ui` and `desktop/ui` (regenerated `stateMatrix.generated.ts` after the body edits).
- Unit tests: 11 pass (added `canPrevPage`/`canNextPage`).
- Backend: `cargo run --generate-openapi` recompiled clean for both binaries; integration test target recompiles.

## Re-audit (full blind round on the fix diff)

A fresh blind fork reviewer (correctness / security / perms-authz /
resource-lifecycle / a11y / api-contract / tests-quality) re-reviewed the fix
changes. It verified every fix sound (get_raw→FilesDownload + 403; preview-only→403
test; setDocument(null) is pdf.js's documented reset; stepPage double-click-safe;
canNextPage correct at numPages=0; state-reset runs once per doc) EXCEPT it caught
one NEW low finding introduced by the a11y fix: the newly-focusable container used
`focus:outline-none` with no replacement indicator (WCAG 2.4.7). Carried to FIX_ROUND-2.

**New confirmed findings:** 1
