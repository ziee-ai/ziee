# DRIFT-1 — implementation vs plan

Audit of the implemented diff against PLAN.md / DECISIONS.md / TESTS.md. Every ITEM
was implemented; the divergences below are refinements forced by the codebase
(iframe reachability, a lint guard) or by following existing precedent, and each is
reconciled by amending the plan artifacts (impl-wins).

## ITEM coverage check (all present)

- ITEM-1 ✓ `File.store` `imageViewStates` + `setImageViewMode`/`zoomImage`/`resetImageView`, sync-drop + reconnect-clear.
- ITEM-2 ✓ `image/header.tsx` zoom-out / fit⇄100% Segmented / zoom-in (right-panel only).
- ITEM-3 ✓ `image/body.tsx` `ImagePanelBody` — fit vs actual, transform scale, pointer-drag pan clamped via `clampTranslate`.
- ITEM-4 ✓ `find/{useFindInDocument,FindBar,FindableRegion,matcher,highlightSupported}` — Highlight API, count + next/prev, Ctrl-F, MutationObserver rebuild.
- ITEM-5 ✓ `fileFindOpen` + `FindButton`; feature-detect hides it when unsupported.
- ITEM-6 ✓ `fileWordWrap` + `WrapToggle`; `RawCodeView` `wordWrap` prop + CSS.
- ITEM-7 ✓ `CopySelectionButton`.
- ITEM-8 ✓ text/markdown/web headers + bodies wired.
- ITEM-9 ✓ `OpenInNewTabButton` in `FilePanelHeaderActions`.
- ITEM-10 ✓ `FileViewPage` + `/files/:fileId` route + `FullPageButton`.
- ITEM-11 ✓ coverage.ts + stateCoverage.ts entries + regen; `viewer-affordances.spec.ts`.

## Drifts

- **DRIFT-1.1** — verdict: impl-wins — DEC-11 said Markdown & Web get Find + Copy-selection "always". Implemented: the **web** viewer exposes Find / Word-wrap / Copy-selection only in RAW mode, because the rendered web branch is a sandboxed `<iframe srcDoc>` — a separate document our CSS-Highlight / `window.getSelection()` cannot reach, so those controls would be dead over it. Markdown-rendered (same-document Streamdown) keeps find always. DECISIONS.md DEC-11 amended.
- **DRIFT-1.2** — verdict: impl-wins — DEC-12 chose `FileOutput` for open-in-new-tab (matching InlineFilePreview). The `lint:icon-action` guard mandates `ExternalLink`/`SquareArrowOutUpRight` for a "new tab" action and failed the build; switched to `ExternalLink`. DECISIONS.md DEC-12 amended.
- **DRIFT-1.3** — verdict: impl-wins — DEC-14 planned a new `gallery-page-file-view` + seeded FilePreviewDrawer variants. Implemented instead by following the EXISTING file-module coverage precedent: `FileViewPage` → `static` (e2e-verified, exactly like the sibling FilePreviewDrawer), `FindBar`/`FindableRegion` → `via`, `FileViewPage:delayed` allow-listed in `stateCoverage.ts`; the functional flows are covered by the real-backend e2e (TEST-5/6/7/10) and the shell chrome by `gate:ui` + `viewer-affordances.spec.ts`. Adding a seeded page would require mock file-content responses the module has never needed. PLAN ITEM-11, DEC-14, TESTS TEST-11 amended. `npm run check` (ui) passes green with these entries.
- **DRIFT-1.4** — verdict: resolved — `clampScale` refined so `+Infinity` saturates at `MAX_SCALE` (not `MIN_SCALE`); matches the documented intent and the unit test (TEST-1). No plan change needed.
- **DRIFT-1.5** — verdict: none — `image/body.tsx` was split into an inner `ImagePanelBody` component so the zoom/pan hooks aren't conditional on the inline branch (rules-of-hooks). Pure implementation structure; plan unaffected.

**Unresolved drifts:** 0
