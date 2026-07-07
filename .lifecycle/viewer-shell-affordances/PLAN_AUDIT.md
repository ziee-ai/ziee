# PLAN_AUDIT — viewer-shell-affordances

Audit of PLAN.md against the codebase. Facts verified against the worktree
(`/data/pbya/ziee/tmp/f2-wt`).

## Breakage risk

- **Existing behaviour preserved by default.** Every new capability is an
  additive off-by-default state: `imageViewStates`/`fileWordWrap`/`fileFindOpen`
  are absent for existing files → defaults reproduce today's render (image
  `{scale:1, mode:'fit'}` == current `object-contain`; `wordWrap` absent == current
  `white-space: pre`; find closed == no bar). `RawCodeView`'s new `wordWrap?` prop
  is optional → its two current callers (text, web-raw) and markdown-raw compile
  unchanged until passed.
- **Inline (`{source}`) path untouched.** All new chrome is gated behind
  `'file' in props` / a real `fileId`, exactly like the existing `RawToggle`/
  `CopyButton`. `InlineFilePreview` and the chat-inline dispatch don't change.
- **`ImageBody` inline branch** (the `if (!file)` external-MCP `<img>`) is left
  byte-for-byte; only the right-panel branch gains transform/pan. No regression to
  the chat inline image.
- **New route `/files/:fileId`** is additive; no existing route uses that prefix
  (`grep` of module route tables shows `/files` is unclaimed — files had "No routes"
  until now). `FileViewPage` reuses `FilePanel`, so no viewer logic is duplicated or
  forked.
- **`FilePanelHeaderActions`** gains OpenInNewTab + FullPage buttons for ALL files;
  these are pure additions to the action row (drawer footer + non-chat right-panel
  title bar). The too-large-file branch still shows plain Download (unchanged).

## Pattern conformance

- **Store maps** mirror `File.store.ts::fileViewModes` verbatim — Map in `state`,
  immutable-copy setter in `actions`, per-file drop in `onFileSync`, full clear in
  `onReconnect`. `defineStore('File', { state, actions, init })` shape confirmed
  (store-kit); handlers read `Stores.File.__state` not the render proxy
  ([[feedback_stores_state_in_handlers]]).
- **Chrome buttons** mirror `chrome.tsx::CopyButton`/`DownloadButton` (ghost icon,
  tooltip, `data-testid`). Headers mirror `text/header.tsx` `Space` composition +
  the `if (!('file' in props)) return null` guard.
- **Route/page** mirror `chat/module.tsx` route entries (`AppLayoutDef`,
  `requiresAuth`) — import path `@/modules/layouts/app-layout` confirmed;
  `useNavigate` (react-router-dom) is the in-repo nav idiom.
- **Find** introduces a new dependency (CSS Custom Highlight API) — no existing
  in-repo precedent (grep found none), so it's a genuinely new primitive; the
  feature-detect + native-find fallback keeps it from regressing unsupported
  browsers. Justified: it's the only DOM-mutation-free way to highlight over shiki
  output + `content-visibility` virtualization without breaking Streamdown's render.

## Migration collisions

- **None.** Zero backend files touched (`src-app/server/**`, `src-app/desktop/tauri/**`
  untouched); no new SQL migration; `ls migrations` unchanged. This is a
  frontend-only diff.

## OpenAPI regen

- **Not required.** No Rust type / route / schema change → `openapi.json` +
  `api-client/types.ts` are untouched in BOTH `ui` and `desktop/ui`. `ApiClient.File.get`
  (`GET /api/files/{file_id}`) and `File.generateDownloadToken` already exist in the
  committed client (verified in `src/api-client/types.ts`), so ITEM-10/ITEM-9 consume
  existing endpoints. Because no generated file changes, the phase-3/phase-8 UI gates
  key off the real `ui/**` source edits (correct — this IS UI work).

## Per-item verdicts

- **ITEM-1** — verdict: PASS — `imageViewStates` map + setters mirror `fileViewModes`; additive, defaults reproduce current render.
- **ITEM-2** — verdict: PASS — right-panel `ImageHeader` gains zoom chrome; inline guard unchanged; `ZoomIn`/`ZoomOut`/`Segmented` all exist.
- **ITEM-3** — verdict: CONCERN — pan/zoom + drag is the most novel body logic; must clamp translate to bounds and reset on fit, and keep the inline `<img>` branch untouched. Not blocking (self-contained, well-scoped) but the highest-attention item for the phase-6 audit.
- **ITEM-4** — verdict: CONCERN — CSS Custom Highlight API is new to the repo; TS types (`Highlight`/`HighlightRegistry`/`CSS.highlights.set`/`.add`) verified to compile under the project's exact `lib: [ES2020, DOM, DOM.Iterable]`. Must clear stale highlights on unmount/query-change and rebuild ranges when Streamdown re-renders. Feature-detected; not blocking.
- **ITEM-5** — verdict: PASS — `fileFindOpen` map + `FindButton` mirror the store/chrome idiom; Ctrl-F interception scoped to the `FindableRegion` (no global key hijack); hidden when Highlight API absent.
- **ITEM-6** — verdict: PASS — `wordWrap` is an optional `RawCodeView` prop + a store map; toggling `.line-code` white-space is a contained CSS change; existing callers unaffected.
- **ITEM-7** — verdict: PASS — `CopySelectionButton` reads `window.getSelection()`; no store state; warns on empty/outside selection.
- **ITEM-8** — verdict: PASS — pure wiring of the new chrome/wrappers into the 3 viewers' headers+bodies; each keeps its inline guard and loading/error branches.
- **ITEM-9** — verdict: PASS — reuses the existing `openFileInNewTab` store action; button added to the shared shell action row.
- **ITEM-10** — verdict: CONCERN — adds the module's FIRST route + a page + loading/not-found states. Reuses `FilePanel` (no viewer fork). Must handle the fetch error/404 and drawer-close-on-navigate cleanly. Not blocking (route shape mirrors chat exactly).
- **ITEM-11** — verdict: CONCERN — gallery/state-matrix coverage is REQUIRED for the phase-8 UI gates (`check:state-matrix`, `check:gallery-coverage`, runtime-health, visual). Every new conditional state (image-zoomed, find-open, wrap-on, file-view page loaded/not-found) needs a gallery cell or an allowlist reason. Generated files (`*.generated.*`) must be regenerated. Not blocking but load-bearing for phase 8 — budgeted here so it isn't discovered late.
