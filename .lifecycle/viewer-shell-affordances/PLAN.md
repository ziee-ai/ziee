# PLAN — viewer-shell-affordances

Cross-viewer UX affordances (F2 in `CAPABILITY_FEATURE_GROUPS.md`; clusters 1c/1d/1e
in `CAPABILITY_AUDITED.md`). All frontend, all in the `file` module. No backend, no
migration, no OpenAPI change — every new capability is client-side chrome over the
existing viewer/store/route surface.

Three areas:
- **Image viewer** — zoom in/out, fit / 100% (actual), pan-when-zoomed.
- **Text / Markdown / Web viewers** — find-in-document (Ctrl-F, highlight + next/prev),
  word-wrap toggle, copy-selection (copy-all already exists).
- **Viewer shell** — open-in-new-tab + a dedicated full-page file view.

## Items

- **ITEM-1**: Add per-file **image view state** to `File.store` — `imageViewStates: Map<fileId, { scale: number; mode: 'fit' | 'actual' }>` plus actions `setImageViewMode(fileId, mode)`, `zoomImage(fileId, factor)` (multiplies scale, clamps 0.1–8, switches mode→'actual'), and `resetImageView(fileId)`. Entry dropped in `onFileSync` and cleared in `onReconnect` alongside `fileViewModes` (mirrors that exact idiom). Default when absent: `{ scale: 1, mode: 'fit' }`.
- **ITEM-2**: `ImageHeader` (right-panel `{file}` context only) renders zoom chrome — zoom-out / zoom-in icon buttons + a `Segmented` fit⇄100% toggle — reading/writing `imageViewStates` via the store. Inline (`{source}`, no FileEntity) context renders nothing new (unchanged).
- **ITEM-3**: `ImageBody` (right-panel) applies the store scale/mode: `mode:'fit'` = `object-contain` sized to the panel (current behaviour, scale 1); `mode:'actual'`/scale≠1 = intrinsic-size image inside a scroll/overflow container transformed by `scale`, **pannable by pointer-drag** when it overflows (local `translate` state, clamped to content bounds, cursor `grab`/`grabbing`, reset when returning to fit). Inline `<img>` path unchanged.
- **ITEM-4**: Shared **find-in-document** capability under `viewers/shared/find/` — a `useFindInDocument(containerRef, query, active)` hook built on the **CSS Custom Highlight API** (registers `file-find` + `file-find-active` highlights over `Range`s walked in DOM order; exposes `{ count, activeIndex, next, prev }`; scrolls the active match into view). It mutates no DOM, so it cooperates with shiki output, Streamdown re-renders, and `content-visibility` virtualization. A `FindBar` component (query `Input`, "n / m" count, prev/next/close buttons, Enter=next, Shift+Enter=prev, Esc=close) and a `FindableRegion` wrapper that hosts the bar over its children, captures **Ctrl/Cmd-F** inside the region, and reads/writes the store open-flag (ITEM-5).
- **ITEM-5**: Add find open-state coordination to `File.store` — `fileFindOpen: Map<fileId, boolean>` + `setFileFindOpen(fileId, open)`, cleared in `onReconnect`. A `FindButton` chrome component toggles it. `find/highlightSupported.ts` feature-detects the Highlight API; `FindButton`/`FindBar` render nothing (and Ctrl-F is not intercepted) when unsupported, so browsers without it fall back to native find.
- **ITEM-6**: **Word-wrap** — `File.store` `fileWordWrap: Map<fileId, boolean>` + `setFileWordWrap(fileId, on)` (cleared/dropped like `fileViewModes`). `RawCodeView` accepts a `wordWrap?: boolean` prop that switches `.line-code` `white-space` `pre`→`pre-wrap` (and drops the horizontal scroll / `width: max-content`) when on. A `WrapToggle` chrome button (icon `WrapText`) drives it; shown for code/raw contexts.
- **ITEM-7**: `CopySelectionButton` chrome — copies `window.getSelection()`'s text when the selection is inside the viewer region (`message.warning` when the selection is empty/outside). Sits next to the existing copy-all `CopyButton`.
- **ITEM-8**: Wire the new chrome into the three text-ish viewers. Bodies: `TextBody` + `WebBody`(raw) wrap `RawCodeView` in `FindableRegion` and pass `wordWrap`; `MarkdownBody` wraps both its raw (`RawCodeView`) and rendered (`Streamdown`) output in `FindableRegion` and passes `wordWrap` to the raw path. Headers (`text/header.tsx`, `markdown/header.tsx`, `web/header.tsx`): add `FindButton` + `CopySelectionButton` (all three) and `WrapToggle` (text always; markdown/web only in raw mode).
- **ITEM-9**: `OpenInNewTabButton` chrome (reuses the existing `File.store.openFileInNewTab` → download-with-token raw view). Added to `FilePanelHeaderActions`' action row so it appears in the `FilePreviewDrawer` footer and the (non-chat) right-panel title bar for every file type — one shared shell affordance, not per-viewer.
- **ITEM-10**: **Dedicated full-page file view** — a `FileViewPage` (`components/FileViewPage.tsx`) that reads the `:fileId` route param, fetches the `FileEntity` via `ApiClient.File.get` (loading / not-found states), and renders `FilePanel` full-screen with a back button. Register route `/files/:fileId` in `file/module.tsx` (module already `dependencies: ['router']`), `requiresAuth`, `AppLayoutDef`. A `FullPageButton` chrome navigates there (via the router) and closes the `FilePreviewDrawer`; added to the same shell action row as ITEM-9.
- **ITEM-11**: Gallery/state coverage for the new surfaces so `check:state-matrix` / `check:gallery-coverage` pass — following the existing file-module precedent (see DEC-14 / DRIFT-1.3): classify `FileViewPage` as `static` (e2e-verified, like the sibling `FilePreviewDrawer`), `FindBar` / `FindableRegion` as `via`, and allow-list the `FileViewPage:delayed` route-lazy state in `stateCoverage.ts`; regenerate the `*.generated.*` artifacts. Backend-free rendering of the new shell chrome is covered by `gate:ui` runtime-health + a gallery-driven `viewer-affordances.spec.ts` over the existing file-preview overlay (no new seeded page needed).

## Files to touch

New:
- `src-app/ui/src/modules/file/viewers/shared/find/useFindInDocument.ts`
- `src-app/ui/src/modules/file/viewers/shared/find/FindBar.tsx`
- `src-app/ui/src/modules/file/viewers/shared/find/FindableRegion.tsx`
- `src-app/ui/src/modules/file/viewers/shared/find/highlightSupported.ts`
- `src-app/ui/src/modules/file/components/FileViewPage.tsx`

Edit:
- `src-app/ui/src/modules/file/stores/File.store.ts` (ITEM-1/5/6 state + actions + sync/reconnect clearing)
- `src-app/ui/src/modules/file/viewers/image/header.tsx` (ITEM-2)
- `src-app/ui/src/modules/file/viewers/image/body.tsx` (ITEM-3)
- `src-app/ui/src/modules/file/viewers/shared/chrome.tsx` (ITEM-5/6/7/9/10 chrome buttons)
- `src-app/ui/src/modules/file/viewers/shared/RawCodeView.tsx` (ITEM-6 wordWrap prop)
- `src-app/ui/src/modules/file/viewers/text/{header,body}.tsx` (ITEM-8)
- `src-app/ui/src/modules/file/viewers/markdown/{header,body}.tsx` (ITEM-8)
- `src-app/ui/src/modules/file/viewers/web/{header,body}.tsx` (ITEM-8)
- `src-app/ui/src/modules/file/components/FilePanel.tsx` (ITEM-9/10 shell action row wiring)
- `src-app/ui/src/modules/file/module.tsx` (ITEM-10 route)
- `src-app/ui/src/dev/gallery/overlays.tsx` + `src-app/ui/src/dev/gallery/pages.tsx` (+ regenerated `*.generated.*` / coverage) (ITEM-11)
- e2e specs under `src-app/ui/tests/e2e/` (see TESTS.md)

## Patterns to follow

- **Store view-state maps** — mirror `File.store.ts::fileViewModes` EXACTLY: a `Map<fileId, …>` in `state`, an immutable-copy setter action, membership in the `onFileSync` per-file drop + `onReconnect` full clear. New maps (`imageViewStates`, `fileFindOpen`, `fileWordWrap`) copy that idiom verbatim ([[feedback_match_existing_patterns]]).
- **Chrome buttons** — mirror `viewers/shared/chrome.tsx::CopyButton` / `DownloadButton`: `Button variant="ghost" size="icon"`, `tooltip`, `data-testid="file-viewer-<x>-btn"`, read/write `Stores.File.__state` in handlers (never the render proxy — [[feedback_stores_state_in_handlers]]).
- **Header composition** — mirror `text/header.tsx` / `markdown/header.tsx`: `Space size="small"` of chrome buttons, `if (!('file' in props)) return null` inline guard.
- **Full-page route + page** — mirror the chat route entries in `chat/module.tsx` (`path`, `element`, `requiresAuth`, `layout: AppLayoutDef`); the page reuses `FilePanel` (already `h-full`) rather than re-implementing a viewer.
- **Gallery** — mirror the existing `overlays.tsx` `overlay-file-preview-drawer` entry and `pages.tsx` page-entry convention (`gallery-page-<id>`, `data-gallery-state`).
- **e2e** — mirror `tests/e2e/visual/overlays.spec.ts` (gallery-driven, backend-free `openGallery` + open-overlay + assert) for the viewer-chrome interaction specs; use the standard real-backend harness only where a route/nav flow needs it.
