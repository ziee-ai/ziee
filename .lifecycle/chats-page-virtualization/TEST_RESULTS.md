# TEST_RESULTS — chats-page-virtualization

Scoped to the touched area (frontend-only: `src-app/ui`). Full logs under
`/data/pbya/ziee/tmp/lifecycle-logs/chats-virt-*.log`.

**Re-run against the MERGED base** (origin/main `e2b5bba3e`, incl. live4's
sidebar-recent infinite-paging store): unit 20/20, visual e2e 4/4
(`chats-virt-visual-merged.log`), real-path + regression e2e 5/5
(`chats-virt-realpath-merged.log`) — virtualization composes with live4's paged
`conversations` cursor (Load-More pages into it; the sidebar uses a separate
`recentConversations` cursor).

## Unit (node:test) — `chats-virt-unit`

- **TEST-1**: PASS — estimator totality, long>short, message_count monotonic (strict), width-sensitivity (strict), 2-line cap, per-bucket stability.
- **TEST-2**: PASS — measured-height cache reused for conversation UUIDs (round-trip, seed omits uncached, stale-width miss).
- **TEST-3**: PASS — `makeChatListMetrics` live counter view, reset, totalSize passthrough.

## E2E — visual gallery (`playwright.visual.config.ts`) — `chats-virt-visual.log`

- **TEST-4**: PASS — only a WINDOW of rows mounted (≪ 200); totalSize reflects all 200.
- **TEST-5**: PASS — scrolling to the bottom detaches the first row + mounts the last; scroll-to-top reverses it.
- **TEST-6**: PASS — no corrections while idle after a cold deep scroll; totalSize stable at rest; c1 < 50; zero console/page errors.
- **TEST-12**: PASS — narrow (390px column) surface also windows rows + stays jank-free at rest.

## E2E — real `/chats` path (default config, full stack) — `chats-virt-realpath.log`

- **TEST-7**: PASS — per-row interactions survive virtualization (scroll-out-and-back re-mounts the same row with the same content; card click navigates to `/chat/{id}`).
- **TEST-8**: PASS — 120 conversations loaded via Load-More, DOM mounts a bounded window (≪ 120) — virtualization holds on the real page + composes with paging.

## E2E — regression (unchanged specs re-run) — `chats-virt-regression.log`

- **TEST-9**: PASS — Load-More still fetches the next page + updates "Showing N of M".
- **TEST-10**: PASS — server-side search still filters + renders results / no-match empty state.

## Static gate (Phase 8) — `chats-virt-*`

- **TEST-11**: PASS — new gallery surfaces (wide + narrow) + `chat-conversation-list-scroll` testid registered; `check:gallery-coverage` + `check:state-matrix` + `check:testid-registry` green.
- **npm run check (ui): PASS** — tsc + biome guardrails + lint:colors/settings-field + check:kit-manifest/testid-registry/design-spec/gallery-coverage/state-matrix/overlay-registry/override-registry.
- **gate:ui (ui): PASS** — A7 boot/runtime canary for the TOUCHED surfaces:
  `runtime-health --only-match=conversation-list-long` exits **RC=0** with
  **0 HIGH / 0 MEDIUM / 4 LOW** (LOW = spacing-grid drift, informational/
  non-gating) across the wide + narrow surfaces (`chats-virt-rh-scoped.log`); tsc
  + lint clean. NOTE: the repo-wide `npm run gate:ui` exits non-zero on
  PRE-EXISTING, unrelated surfaces already failing on `main` (settings-general,
  knowledge, file-rag-admin, deep-chat-*, filecard-*, citations, voice,
  llm-models, provider-api-key-modal, group-widget) — NONE of whose source files
  are in this diff (`git diff origin/main...HEAD --name-only` confirms). That
  pre-existing debt is out of scope for this feature; the touched-surface canary
  is genuinely clean.

Notes:
- No backend diff → backend integration chain N/A. No new permission → A9/A10 N/A.
- Only workspace touched is `src-app/ui` (desktop/ui shares the chat module; no override).
