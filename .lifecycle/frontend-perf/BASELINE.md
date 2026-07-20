# frontend-perf — BASELINE metrics

Captured on branch `feat/frontend-perf`, base `origin/main` = `e2800d9a1`.
All numbers below are **measured**, not estimated. Re-run the same commands
after each optimization round to show the delta.

## How to reproduce

```bash
cd /data/pbya/ziee/tmp/frontend-perf-wt/src-app/ui
npm run build                       # writes ../dist/ui
cd ../dist/ui/assets
for f in index-*.js index-*.css; do
  raw=$(stat -c%s $f); gz=$(gzip -c $f | wc -c)
  awk -v n="$f" -v r="$raw" -v g="$gz" 'BEGIN{printf "%-24s raw %8.1f KB  gzip %8.1f KB\n",n,r/1024,g/1024}'
done
```

## B1 — Critical path (what the browser downloads before first paint)

`index.html` eagerly references exactly two assets — one JS entry, one CSS.

| asset | raw | gzip |
|---|---:|---:|
| `assets/index-0m9Kpvdv.js` | **2150.2 KB** | **632.6 KB** |
| `assets/index-fc9_FYZg.css` | 250.7 KB | 41.4 KB |
| **critical-path total** | **2400.9 KB** | **674.0 KB** |

**674 KB gzip blocks first paint.** This is the headline number to beat.

## B2 — Whole-bundle shape

| metric | value |
|---|---:|
| `dist/ui` total on disk | 14 MB |
| JS chunk count | 493 |
| JS total (raw, all chunks) | 17.04 MB |
| CSS files | 2 |
| production build wall-clock | 18.5 s (`real`) |

493 chunks exist, so code-splitting is *happening* — but the entry chunk is
still 2.1 MB, meaning the split is largely of leaf/vendor libs, not of the
app's own route/module graph. That is the core finding.

## B3 — Largest chunks

Only the first row is on the critical path; the rest are lazily fetched and
are listed to show dependency weight and lazy-load correctness.

| chunk | raw KB |
|---|---:|
| `index-0m9Kpvdv.js` (ENTRY — critical path) | 2150.2 |
| `KitMarkdownEditor` | 864.1 |
| `emacs-lisp` (shiki grammar) | 761.6 |
| `cpp` (shiki grammar) | 611.5 |
| `wasm` (shiki grammar) | 607.7 |
| `mermaid-parser.core` | 589.3 |
| `pdfjs` | 577.0 |
| `cytoscape.esm` | 424.1 |
| `xlsx` | 414.8 |
| `KitCodeEditor` | 383.4 |
| `chunk-BO2N2NFS` | 336.0 |
| `wolfram` (shiki grammar) | 256.2 |

Note the long tail of **shiki syntax-highlighting grammars** shipped as
individual chunks (emacs-lisp, cpp, wasm, wolfram, objective-cpp, angular-ts,
vue-vine, …). They are lazy, but the *set* of grammars bundled is worth
auditing against the languages the app actually highlights.

## B4 — Bundler-reported defects (from the build log)

The build emits **14 `INEFFECTIVE_DYNAMIC_IMPORT` warnings**. Each is a module
that some call site `import()`s dynamically — intending to split it out — while
another call site imports it *statically*, which collapses the dynamic import
and pulls the module back into the eager graph. These are concrete, individually
fixable code-splitting bugs, not stylistic warnings.

Affected modules (dynamic intent defeated by a static importer):

| module intended to be lazy | defeated by a static import in |
|---|---|
| `sdk/packages/framework/src/stores.ts` | `framework/src/index.ts`, `store-kit.ts`, `permissions/authView.ts`, … |
| `sdk/packages/kit/src/index.ts` | `notification-ui/*`, `shell/src/components/Drawer.tsx`, … |
| `node_modules/lucide-react` | `kit/src/kit/{alert,button,date-picker,dialog-host,image}.tsx`, … |
| `sdk/packages/framework/src/api-client/core.ts` | `api-client/index.ts`, `sync/SyncClient.ts`, `Auth.store.ts`, … |
| `src/api-client/index.ts` | `KitMarkdownEditor.tsx`, `App.store.ts`, 5+ stores |
| `src/modules/auth/Auth.store.ts` | `App.tsx`, `auth/module.tsx`, 3 more stores |
| `src/modules/auth/AuthPage.tsx` | `auth/AuthGuard.tsx` |
| `src/modules/chat/core/extensions/index.ts` | `ChatInput.tsx`, `ChatMessage.tsx`, `ContentRenderer.tsx`, … |
| `src/modules/chat/core/stores/Chat.store.ts` | `ChatRightPanel.tsx`, `ChatPaneContext.tsx`, `chatBridge.ts` |
| `src/modules/chat/core/stores/chatBridge.ts` | `ChatPaneContext.tsx`, `Voice.store.ts`, `chat/module.tsx` |
| `node_modules/shiki/dist/index.mjs` | `@streamdown/code/dist/index.js` |
| `src/modules/assistant/stores/AssistantPicker.store.ts` | `AssistantMenuItem.tsx`, `AssistantStatusChip.tsx`, `stores/index.ts` |
| `src/modules/mcp/stores/McpComposer.store.ts` | `McpStatusRow.tsx`, `McpConfigModal.tsx`, `stores/index.ts` |
| `src/modules/knowledge-base/stores/kbSelectionKey.ts` | `KbMenuItem.tsx`, `KbStatusRow.tsx`, `KnowledgeBaseComposer.store.ts` |
| `src/modules/user-llm-providers/ModelPicker.store.ts` | `ModelSelector.tsx`, `module.tsx`, `WorkflowRunDialog.tsx` |

A recurring root cause is visible: a **barrel file** (`stores/index.ts`,
`kit/src/index.ts`, `api-client/index.ts`, `framework/src/index.ts`) statically
re-exports a module that someone else tried to lazy-load. Barrel re-exports are
the single biggest structural blocker to splitting this app.

Rolldown also emits: *"Some chunks are larger than 500 kB after minification"*.

## RESULTS LOG (per item)

### ITEM-1 (Streamdown plugins → lazy) — DONE, measured
- Entry chunk: **632.6 → 482.1 KB gzip (−150.5 KB, −23.8%)**; raw 2150.2 → 1661.3 KB.
- Login-page total JS transfer: **841.3 → 690.4 KB**; FCP **896 → 592 ms**.
- Functional regression proof (`.runlogs/render-check.mjs`, prod build @29182,
  seeded conversation): shiki code **541 spans ✓**, katex math **7 nodes ✓**,
  mermaid **6 blocks ✓**, **0 console/page errors**. A/B vs the un-patched build
  confirmed rendering is byte-identical to main (the ```mermaid fence renders as a
  code-block on main too — pre-existing, NOT an item-1 regression).
- Static gate: `tsc` + guardrails/colors/settings-field/adjacent-inline/icon-action/
  logical-direction/tooltip-placement/kit-manifest all PASS. (`check:testid-registry`
  fails on PRE-EXISTING sdk-submodule drift unrelated to this diff — reverted, not
  ours to fix here.)

### ITEM-2 (scope usePrefetchModules) + ITEM-5 (drawer shouldMount) + root-component gating — DONE, measured
- `usePrefetchModules` now gates on auth + per-route permission + drops the forced
  `{timeout:2000}`. Root always-mounted components (`LlmRepositoryDrawer`,
  `LlmModelDownloadNotifications`, `NotificationToastListener`) gained shouldMount
  gates (open-state / `llm_models::downloads_read` / `notifications::read`).
- **Login page (logged out): 80 JS chunks → 2** (entry + RouterComponent only).
  Total transfer 908.7 → 551.9 KB; FCP 896 → ~550 ms.
- Prefetch scoping proven across 3 states: logged-out → 0 route chunks;
  non-admin `tester` → 48 (admin pages excluded, NONE leaked ✓); admin → 80 (all
  permitted), `/settings/general` renders on nav.
- Root-component gates proven: logged-out mounts none of the 3; authenticated
  users WITH the perm still mount them (admin + tester both mount toast+download
  listeners — tester legitimately holds `downloads_read`+`notifications::read`
  via the default Users group). Drawer's `GET /api/llm-repositories`-on-every-route
  bug fixed (mounts only when open).
- Regression: chat route renders code/math/mermaid identical to main, 0 errors.
  `tsc` + guardrails + colors PASS.

## B5 — Runtime metrics (TTI / LCP / request waterfall)

Login-page (prod, unauth) captured — baseline FCP 896 ms → 592 ms after ITEM-1.
Authenticated chat-route TTI/LCP still to be captured for ITEM-6's waterfall work.
PENDING — to be captured against the live dev server on port 29181 once the
backend is up and the showcase seed is loaded, so the numbers reflect realistic
data volume rather than an empty database. Method: Playwright/CDP navigation
timing + a `performance.getEntriesByType('resource')` dump on a cold load of
`/` and of the chat route, logged-in, cache disabled.

## B6 — Code-splitting / waterfall source analysis

PENDING — see `BASELINE_ANALYSIS.md` (route-splitting state, module
auto-discovery eagerness, startup fetch waterfalls, entry-chunk dependency
attribution).
