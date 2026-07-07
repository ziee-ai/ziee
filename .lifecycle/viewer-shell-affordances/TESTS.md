# TESTS — viewer-shell-affordances

Frontend-only diff → the UI e2e gate applies. Tiers used:
- **unit** — `node --test` over pure `.ts` logic (repo convention:
  `src/modules/chat/core/tool-status.test.ts`). No DOM/JSX — so each testable
  behaviour is extracted into a pure helper (`image/zoom.ts`, `find/matcher.ts`,
  `find/highlightSupported.ts`) that the components consume.
- **e2e** — Playwright. Interactive viewer-chrome flows run against the **real
  backend** (`loginAsAdmin` + API upload → project-knowledge `FileCard` → click →
  `FilePreviewDrawer`, the real dispatch path, mirroring
  `tests/e2e/projects/attach-file.spec.ts`). The new-state rendering / runtime-health
  / contrast coverage (ITEM-11) runs **backend-free** against the gallery
  (`tests/e2e/visual/`, mirroring `overlays.spec.ts`).

No cosmetic tests — the e2e specs drive the production component through a real
render + real store; only the LLM boundary is never involved (these viewers don't
call it). ([[feedback_no_cosmetic_tests]])

## Tests

- **TEST-1** (tier: unit) [covers: ITEM-1] file: `src-app/ui/src/modules/file/viewers/image/zoom.test.ts` — asserts: `clampScale` clamps to [0.1, 8]; `zoomStep(scale, factor)` multiplies then clamps and never returns 0/NaN; the default view state is `{scale:1, mode:'fit'}`.
- **TEST-2** (tier: unit) [covers: ITEM-3] file: `src-app/ui/src/modules/file/viewers/image/zoom.test.ts` — asserts: `clampTranslate(tx,ty,bounds)` pins pan within `[-max,max]` per axis and returns `{x:0,y:0}` when the scaled content fits the container (no overflow → no pan).
- **TEST-3** (tier: unit) [covers: ITEM-4] file: `src-app/ui/src/modules/file/viewers/shared/find/matcher.test.ts` — asserts: `collectMatches(haystack, needle)` is case-insensitive, returns non-overlapping `{start,end}` ranges in ascending order, yields `[]` for an empty/whitespace needle, and the count equals the number of occurrences.
- **TEST-4** (tier: unit) [covers: ITEM-5] file: `src-app/ui/src/modules/file/viewers/shared/find/highlightSupported.test.ts` — asserts: `isHighlightSupported()` returns a boolean and never throws, and is `false` in the non-DOM node env (no `CSS.highlights`) — proving the fallback path is taken when the API is absent.
- **TEST-5** (tier: e2e) [covers: ITEM-1, ITEM-2, ITEM-3] file: `src-app/ui/tests/e2e/file/image-zoom.spec.ts` — asserts: user uploads a PNG, opens its preview, clicks zoom-in → the image body's applied `scale` transform grows and a horizontal scroll/pan region appears; the fit⇄100% `Segmented` switches `object-contain` ↔ actual-size; dragging the zoomed image pans it (translate changes); reset/fit returns to scale 1 with no pan.
- **TEST-6** (tier: e2e) [covers: ITEM-4, ITEM-5, ITEM-8] file: `src-app/ui/tests/e2e/file/find-in-document.spec.ts` — asserts: user uploads a text file, opens preview, opens the find bar (via the Find button AND via Ctrl-F), types a query → the "n / m" count shows the match total, Next/Prev advance the active index (`1 / m` → `2 / m` → wraps), the active match scrolls into view, and Esc closes the bar; the Find button is absent when the Highlight API is unsupported (asserted via a page-init stub deleting `CSS.highlights`).
- **TEST-7** (tier: e2e) [covers: ITEM-6, ITEM-8] file: `src-app/ui/tests/e2e/file/word-wrap.spec.ts` — asserts: user uploads a file with a very long single line, opens preview; with wrap OFF the code region scrolls horizontally (`scrollWidth > clientWidth`); clicking the Wrap toggle removes the horizontal overflow (`scrollWidth ≈ clientWidth`) and re-clicking restores it.
- **TEST-8** (tier: e2e) [covers: ITEM-7, ITEM-8] file: `src-app/ui/tests/e2e/file/copy-selection.spec.ts` — asserts: with clipboard permission granted, the user selects a substring inside the viewer body, clicks Copy-selection → the clipboard holds exactly the selected text; clicking it with no selection surfaces a warning and does not overwrite the clipboard.
- **TEST-9** (tier: e2e) [covers: ITEM-9] file: `src-app/ui/tests/e2e/file/open-in-new-tab.spec.ts` — asserts: clicking Open-in-new-tab opens a popup whose URL is `/api/files/<id>/download-with-token?token=…` (caught via `context.waitForEvent('page')`), proving the existing token-mint path is wired to the new chrome button.
- **TEST-10** (tier: e2e) [covers: ITEM-10] file: `src-app/ui/tests/e2e/file/full-page-view.spec.ts` — asserts: clicking Full-page navigates to `/files/<id>` and closes the drawer; `FileViewPage` renders `FilePanel` (filename + body visible) with a working back button; navigating to `/files/<bogus-uuid>` shows the not-found empty state (no crash).
- **TEST-12** (tier: unit) [covers: ITEM-4] file: `src-app/ui/src/modules/file/viewers/shared/find/offset.test.ts` — asserts: `locateSegment(starts, offset)` maps a global char offset to the containing text segment (boundary → later segment, past-end → last, empty → -1), so a match spanning multiple text nodes resolves its start/end nodes independently (added in Phase 7 to close the cross-node coverage gap).
- **TEST-11** (tier: e2e) [covers: ITEM-11] file: `src-app/ui/tests/e2e/visual/viewer-affordances.spec.ts` — asserts: backend-free (gallery, playwright.visual.config), the FilePreviewDrawer overlay opens with the new shell chrome (`file-viewer-open-tab-btn` + `file-viewer-fullpage-btn`) visible and the dialog layout sane (no horizontal overflow) in light + dark — the stable surface for the phase-8 `gate:ui` runtime-health / AA-contrast pass. (The zoom/find/wrap functional flows are covered against the real backend by TEST-5/6/7; per DRIFT-1.3 no new seeded gallery page is added.)
