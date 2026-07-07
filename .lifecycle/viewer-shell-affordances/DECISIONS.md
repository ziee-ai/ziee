# DECISIONS — viewer-shell-affordances

Every product/UX input the implementation needs, resolved up front so
implementation runs nonstop. All resolvable by existing convention + the audit's
own guidance; none needs a user round-trip.

### DEC-1: Zoom levels, step factor, and clamp range for the image viewer?
**Resolution:** Continuous scale (not fixed stops); zoom-in/out buttons multiply by `1.25` / `0.8`; clamp to `[0.1, 8]`. "Fit" = scale 1 + `object-contain`; "100%" (actual) = render at intrinsic pixel size. `resetImageView` returns to `{scale:1, mode:'fit'}`.
**Basis:** convention — a ±25% step + a wide clamp is the standard image-viewer default; keeping "fit" as the zero-state exactly reproduces the current `object-contain` render so nothing regresses.

### DEC-2: Do zoom controls live in the header or as a floating in-body toolbar?
**Resolution:** Header (`image/header.tsx`), reading/writing `imageViewStates` in `File.store`; the body (`image/body.tsx`) is a pure consumer that applies the transform.
**Basis:** codebase — the scope names both `image/header.tsx` + `image/body.tsx`, and header-button ↔ body coordination through `File.store` is the established pattern (`RawToggle` + `fileViewModes`).

### DEC-3: How is a zoomed image panned — drag, native scroll, or both?
**Resolution:** Both. The zoomed image sits in an `overflow:auto` container (native scrollbars) AND is pointer-drag pannable (cursor `grab`/`grabbing`, local `translate` state clamped to bounds). No wheel-zoom.
**Basis:** convention — drag-to-pan is the expected gesture; keeping native scroll as well is free and accessible. Wheel-zoom is deliberately excluded so the viewer never hijacks page/panel scroll.

### DEC-4: Find matching semantics — case, whole-word, regex?
**Resolution:** Case-insensitive plain-substring, non-overlapping, no regex / no whole-word.
**Basis:** convention — matches the app's other text filters (`ILIKE '%q%'` substring elsewhere); regex/whole-word are additive future options, out of scope for the "table-stakes" find.

### DEC-5: Find highlighting mechanism given shiki + Streamdown + `content-visibility`?
**Resolution:** CSS Custom Highlight API (`CSS.highlights` + `::highlight(file-find)` / `::highlight(file-find-active)` over `Range`s). Feature-detected; when unsupported the Find button + Ctrl-F interception are disabled so the browser's native find takes over.
**Basis:** codebase — it's the only highlight approach that mutates no DOM, so it survives shiki's generated markup, Streamdown re-renders, and the `content-visibility: auto` virtualization in `RawCodeView` (the audit explicitly flags this constraint). TS types verified to compile under the project's `lib: [ES2020, DOM, DOM.Iterable]`.

### DEC-6: Does Ctrl/Cmd-F hijack the browser globally?
**Resolution:** No — the key handler is attached to the `FindableRegion` element and only pre-empts default when the focus/pointer is inside that region (a viewer is open). Everywhere else, native find is untouched.
**Basis:** convention — scoped shortcuts avoid the "app stole my Ctrl-F" anti-pattern; matches how kit overlays scope their key handlers.

### DEC-7: Word-wrap default state and persistence?
**Resolution:** Default OFF (current `white-space: pre` + horizontal scroll). Toggled per-file, held in-memory in `File.store::fileWordWrap` (cleared on sync/reconnect), NOT persisted to the backend.
**Basis:** codebase — mirrors `fileViewModes` (per-file, ephemeral, no server round-trip). OFF preserves today's exact render.

### DEC-8: Copy-selection scope and empty-selection behaviour?
**Resolution:** Copies `window.getSelection()`'s text only when the selection's anchor is inside the viewer region; empty/outside selection → `message.warning('Select text to copy')` and the clipboard is left untouched. Copy-all (`CopyButton`) stays as the whole-document copy.
**Basis:** audit — "Copy-selection button (copy-all already exists)"; warning-not-error on empty matches the app's `message` conventions in `chrome.tsx`.

### DEC-9: Full-page view — route path, layout, auth, data source?
**Resolution:** Route `/files/:fileId`, `requiresAuth: true`, `layout: AppLayoutDef`; `FileViewPage` fetches via `ApiClient.File.get({ file_id })` with a loading spinner and a not-found empty state, then renders `FilePanel` full-screen with a back button.
**Basis:** codebase — mirrors `chat/module.tsx` route entries; `ApiClient.File.get` (`GET /api/files/{file_id}`) already exists; `FilePanel` is already `h-full` and is the single viewer shell, so full-page reuses it (no fork).

### DEC-10: "Open in new tab" — in-app full-page or the raw file?
**Resolution:** The RAW file, via the existing `File.store::openFileInNewTab` (mints a download-token, opens `/api/files/{id}/download-with-token`). The in-app rendered view is the separate Full-page button (DEC-9).
**Basis:** codebase — `openFileInNewTab` already exists and is already used by `InlineFilePreview`; reusing it keeps one token-mint path. The two affordances are distinct and both listed in the audit.

### DEC-11: Which viewers receive which chrome?
**Resolution:** Image → zoom controls only. Text → Find + Word-wrap + Copy-selection (+ existing Copy-all/Download). Markdown → Find + Copy-selection always (rendered Streamdown is same-document, so find reaches it); Word-wrap only in raw mode. **Web → Find + Word-wrap + Copy-selection only in RAW mode** — the rendered branch is a *sandboxed iframe* (a separate document our highlight/selection cannot reach), so these are meaningless over it. Every file type (via `FilePanelHeaderActions`) → Open-in-new-tab + Full-page.
**Basis:** codebase — word-wrap only makes sense over `RawCodeView`'s `pre`; find/selection require a same-document DOM (true for markdown-rendered + all raw views, false for the web iframe); the shell buttons are file-type-agnostic so they belong in the shared action row, not per-viewer. (Refined during implementation — see DRIFT-1.1.)

### DEC-12: Icons (lucide) for the new chrome?
**Resolution:** `ZoomIn` / `ZoomOut` (image), `Search` (find), `WrapText` (word-wrap), `Copy` (copy-all) vs a distinct `ClipboardCopy` (copy-selection), **`ExternalLink` (open-in-new-tab)**, `Maximize2` (full-page). Find-bar prev/next use `ChevronUp`/`ChevronDown`; find-bar close uses `X`.
**Basis:** codebase — all verified present in the installed `lucide-react`. Open-in-new-tab uses `ExternalLink` (not `FileOutput`) because the `lint:icon-action` guard mandates `ExternalLink`/`SquareArrowOutUpRight` for a "new tab" action and fails the build otherwise. (Corrected during implementation — see DRIFT-1.2.)

### DEC-13: Find-bar placement and testids?
**Resolution:** A pinned strip at the top of the `FindableRegion` (query `Input`, "n / m" count with `data-testid="file-find-count"`, prev/next/close). Testids follow the `file-viewer-<x>-btn` / `file-find-*` convention.
**Basis:** codebase — matches the existing `data-testid="file-viewer-*"` naming in `chrome.tsx`; a top strip keeps the content scroll region intact underneath.

### DEC-14: Gallery coverage shape for the new states?
**Resolution:** Follow the EXISTING file-module coverage precedent rather than adding new seeded gallery pages: `FileViewPage` → `{ kind: 'static', reason: '…verified via the e2e interaction suite' }` (exactly how the sibling `FilePreviewDrawer` is classified — both need a seeded file with content); `FindBar` / `FindableRegion` → `{ kind: 'via' }` (rendered within the file viewer body, like `RawCodeView`/`chrome`); the new `FileViewPage:delayed` route-lazy state → an allow-listed `{ skip, reason }` in `stateCoverage.ts`. Regenerate `galleryCoverage.generated.ts` + `stateMatrix.generated.ts`. The zoom-controls / find-open / wrap-on states live *inside* the already-`static` FilePreviewDrawer and are exercised by the real-backend e2e specs; the backend-free `gate:ui` runtime-health pass + the gallery-driven `viewer-affordances.spec.ts` cover the shell chrome rendering. No new seeded page/overlay is added.
**Basis:** codebase — this is precisely how the existing file viewers + FilePreviewDrawer are already covered (via/static + e2e), and adding a seeded gallery page would require standing up mock file-content responses the module has never needed. (Refined during implementation — see DRIFT-1.3.)
