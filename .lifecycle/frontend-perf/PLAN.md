# frontend-perf — PLAN (iterative, living)

Iterative frontend performance + loading-speed optimization. One living plan;
items are worked ONE AT A TIME (implement → rebuild → measure delta on the live
prod-preview → functional e2e → check off → next). New ideas/feedback append as
new ITEM-N / FB-N. A coherent batch of finished items = one merge checkpoint.

Baseline (measured, prod build, `.lifecycle/frontend-perf/BASELINE.md`):
- **critical path = 674 KB gzip** (entry JS 632.6 + CSS 41.4).
- **login page downloads 80 JS chunks / 908.7 KB** (entry 634.4 KB + 79
  speculatively-prefetched route chunks the logged-out user cannot reach).
- FCP 896 ms; 3 serial data tiers before the composer is usable.

Each item states its MEASURED target so the win is provable, not asserted.

## Items

- **ITEM-1**: Move Streamdown plugin construction (`@streamdown/code`→Shiki,
  `@streamdown/math`→KaTeX, `MermaidBlock`) INSIDE the lazy `LazyStreamdown`
  boundary via a `variant: 'chat' | 'base'` prop, so the ~730 KB-raw / ~152 KB-gzip
  katex+shiki graph rides the lazy `streamdown` chunk instead of the entry chunk.
  Call sites stop statically importing `chatMarkdownPlugins`/`STREAMDOWN_PLUGINS`
  and pass `variant` instead. Target: **entry chunk −140 KB+ gzip** (baseline
  632.6 → ≤ ~495 KB gzip), with code/math/mermaid still rendering on the chat
  route. Independent stub-build already proved −152 KB gzip is achievable.
- **ITEM-2**: Gate `usePrefetchModules` (`sdk/packages/shell/src/hooks/usePrefetchModules.ts`)
  so it prefetches ONLY routes the current user is authenticated + permitted to
  reach, excludes the current route, and drops the forced `{timeout: 2000}` (true
  idle only). Target: **login page 80 JS chunks → ≤ 3**, total login JS transfer
  ~841 KB → ~640 KB (entry only).
- **ITEM-3**: Split the eager module-discovery so admin/settings-only module
  manifests (`user`, `auth-providers`, `mcp-admin`, `skills-admin`,
  `workflows-admin`, `summarization-admin`, `file-rag-admin`, `memory-admin`,
  `scheduler` admin) are not baked into the entry chunk for users who can never
  see them. Target: **entry chunk further −N KB gzip** (measured).
- **ITEM-4**: Fix the 14 `INEFFECTIVE_DYNAMIC_IMPORT` warnings — each is a module
  someone tried to lazy-load that a barrel re-export (`stores/index.ts`,
  `kit/index.ts`, `api-client/index.ts`, `framework/index.ts`) pulls back into the
  eager graph. Target: **build log emits 0 ineffective-dynamic-import warnings**;
  unblocks ITEM-3's splits.
- **ITEM-5**: Fix `LlmRepositoryDrawer` registered with no `shouldMount`
  (`modules/llm-repository/module.tsx`) so a CLOSED drawer no longer fetches
  `/api/llm-repositories` on every route for admins. Mirror the sibling drawers'
  `shouldMount: () => useDelayedFalse(...)`. Target: **that request absent from the
  chat-route waterfall** when the drawer is closed.
- **ITEM-6**: Remove a serial boot tier — start the post-login route-chunk preload
  and/or open the sync SSE stream from the already-persisted token instead of
  gating both behind the `/auth/me` round-trip. Target: **measured TTI reduction on
  the authenticated chat-route cold load** (route chunk request starts before
  `/auth/me` resolves).
- **ITEM-7**: Consolidate duplicate deps shipped in the entry chunk — one date lib
  (`date-fns` vs `dayjs`) and audit `@base-ui/react` (559 KB raw, largest single
  contributor) vs the coexisting `@radix-ui/*` (46 KB) for a removable overlap.
  Target: **measured entry-chunk gzip reduction**; no functional change.
- **ITEM-9**: Gate lazy always-mounted ROOT components (ComponentRegistration,
  not routes) on auth/permission via `shouldMount`, so their chunks don't load on
  the logged-out login page (`LlmModelDownloadNotifications` →
  `downloads_read`; `NotificationToastListener` → `notifications::read`). Arose
  from FB-1. Target: login page ≤ 2 JS chunks. DONE.
- **ITEM-8**: Add a committed, merge-durable **bundle/load budget check** (a
  product-tree script, NOT a `.lifecycle` artifact — rule B6) asserting the
  login-page critical path stays within budget (entry JS gzip ≤ threshold, login
  chunk count ≤ threshold), so a regression can't silently creep back. Wired into
  a check the repo already runs.

- **ITEM-10**: Lazy-load the KaTeX (~24 KB) + Streamdown CSS — move the two
  `@import`s out of the eager `src/index.css` into the lazy `streamdownPlugins.ts`
  so Vite splits them into a `streamdownPlugins-*.css` chunk that loads only when
  a `<Streamdown>` mounts. Completes ITEM-1 (JS was lazy, CSS was not). DONE:
  critical CSS **41.4 → 32.8 KB gzip (−8.6 KB, −21%)**; math verified STILL
  STYLED on the chat route (`.katex` → `font-family: KaTeX_Main`), login page
  does not load the math CSS. Arose from a user question about CSS size.

## Pre-existing drift found while working (fix at the END, batched)

These are NOT caused by this feature — they are pre-existing debt on main
surfaced while running the gates here. Recorded so we sweep them in a final
cleanup pass before the checkpoint merge, rather than silently leaving them or
pulling them into an unrelated item's diff.

- **DRIFT-A**: `sdk/packages/kit/src/testIds.generated.ts` (the committed sdk
  submodule pin) is **STALE vs main's own ui/desktop source** — `npm run
  check:testid-registry` fails on a fresh worktree. Regenerating adds
  `chat-pane-*` / `chat-split-btn` / `conversation-picker-*` and removes
  `notification-bell-*` / `notifications-*` (drift from split-chat-multipane +
  notification refactors already merged to main, whose regenerated registry was
  never committed to the sdk pin). Fix: regen `testIds.generated.ts`, commit it
  in the sdk submodule on its dedicated branch, and bump the superproject sdk
  pin — as its OWN change, not folded into a perf item. Reverted locally for now
  so the perf diff stays clean. This blocks `npm run check` at Phase 8, so it
  MUST be resolved (or the sdk pin bumped) before the merge checkpoint.
- (append further pre-existing drift here as found.)

## Files to touch

- ITEM-1: `src/modules/chat/core/utils/LazyStreamdown.tsx`,
  `src/modules/chat/components/TextContent.tsx`,
  `src/modules/chat/extensions/text/components/TextContent.tsx`,
  `src/modules/file/viewers/markdown/body.tsx`,
  `src/modules/skill/components/SkillDetailDrawer.tsx`,
  `src/modules/workflow/components/StepOutputExpander.tsx`.
  (`streamdownPlugins.ts` / `chatMarkdownPlugins.ts` unchanged — they just stop
  being statically reachable.)
- ITEM-2: `sdk/packages/shell/src/hooks/usePrefetchModules.ts` (+ its permission
  helper source).
- ITEM-3/4: `src/modules/loader.ts`, `src/modules/loader.desktop.ts`, the offending
  barrel `index.ts` files, per-module `module.tsx` where a split is introduced.
- ITEM-5: `src/modules/llm-repository/module.tsx`.
- ITEM-6: `src/modules/auth/AuthGuard.tsx`, `sdk/packages/framework/src/sync/index.ts`,
  `sdk/packages/shell/src/bootstrap/AppShell.tsx`.
- ITEM-7: `src-app/ui/package.json` + call sites of the removed dep.
- ITEM-8: a script under `src-app/ui/scripts/` + a package.json `check:*` hook.
- e2e specs: `src-app/ui/tests/e2e/perf/*.spec.ts` (new dir).

## Patterns to follow

- **ITEM-1** mirrors the EXISTING `LazyStreamdown` pattern verbatim
  (`lazyWithPreload` → `React.lazy` → `Suspense` with a plain-text fallback) —
  extend it, don't invent a new lazy mechanism. Keep the desktop-preload contract
  (`lazyWithPreload.desktop.ts`) intact by routing every loader through
  `lazyWithPreload`.
- **ITEM-5** mirrors the sibling drawers that already do it right
  (`modules/file/module.tsx`, `modules/scheduler/module.tsx`): `shouldMount: () =>
  useDelayedFalse(() => Stores.<X>Drawer.open)`.
- **ITEM-2** self-gates on the same `hasPermissionNow`/`evaluatePermission` the
  route guards use (`.claude/PERMISSION_GATING.md`), so prefetch and navigability
  agree.
- Measurement harness: `.runlogs/perf-record.mjs` (Playwright network+timing
  record) is the canonical before/after tool; re-run per item.

## UI-surface checklist

This feature adds/edits NO user-facing surface, page, drawer, card, or panel —
it changes only WHAT loads WHEN (bundling / prefetch / lazy boundaries) behind
byte-identical rendered UI. So the precedent/scale/responsive/JTBD sub-checks are
N/A by construction. The binding requirement instead is **functional-regression
e2e**: every item that changes load timing must prove, on the real prod build,
that the affected route still renders and behaves identically (markdown/code/math
still render, every route still reaches its chunk, no console error / ErrorBoundary
crash). That is enumerated in TESTS.md and run at Phase 8 (`gate:ui` + e2e).
