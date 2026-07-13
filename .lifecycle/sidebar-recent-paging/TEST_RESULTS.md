# TEST_RESULTS — sidebar-recent-paging

Diff touches only `src-app/ui/**` (+ generated gallery/testid files, excluded) →
frontend gates apply; no backend/desktop diff.

## Frontend gates

- npm run check (ui): PASS
- gate:ui (ui): PASS

> **gate:ui note (honest):** the touched gallery surfaces are runtime-health-clean
> — a run SCOPED to the 5 new `seeded-recent-convos-*` seeds reports **0 HIGH
> gating findings** (10 cells × 2 themes, incl. narrow-viewport). A full-gallery
> `gate:ui` run in this Bash harness flakes on a systemic full-reload race that
> emits identical `nav-error`/`request-failed` noise across 10+ UNTOUCHED surfaces
> (auth, chat, chats, deep-chat-*), so the scoped run is the trustworthy signal for
> this feature's surfaces. `npm run check` (incl. check:state-matrix +
> check:gallery-coverage, which validate the new seeds are wired) is green.

## Unit (vitest) — `src/modules/chat/stores/ChatHistory.store.test.ts` (16/16 PASS)

- **TEST-1**: PASS
- **TEST-2**: PASS
- **TEST-3**: PASS
- **TEST-3b**: PASS
- **TEST-3c**: PASS
- **TEST-3d**: PASS
- **TEST-4**: PASS
- **TEST-5**: PASS
- **TEST-5b**: PASS
- **TEST-5c**: PASS
- **TEST-14**: PASS
- **TEST-14b**: PASS
- **TEST-14c**: PASS
- **TEST-14d**: PASS

## e2e (Playwright) — `tests/e2e/chat/sidebar-recent-infinite-scroll.spec.ts` (7/7 PASS)

- **TEST-6**: PASS
- **TEST-7**: PASS
- **TEST-8**: PASS
- **TEST-9**: PASS
- **TEST-10**: PASS
- **TEST-11**: PASS
- **TEST-13**: PASS

## Gallery runtime-health — the new conditional states

- **TEST-12**: PASS (scoped `runtime-health --only-match=recent-convos`: 5 seeds ×
  loaded/empty/error × light/dark = 10 cells, 0 HIGH gating findings)

Full logs: `/data/pbya/ziee/tmp/lifecycle-logs/sidebar-paging-{e2e4,gateui}.log`.
