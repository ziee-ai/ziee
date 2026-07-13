# TESTS — chats-page-virtualization

Every ITEM is covered by ≥1 TEST; every TEST names a valid ITEM, tier, file, and
assertion. UI-touching feature → includes `tier: e2e` specs. **No new permission
is introduced** (reuses existing `conversations::read` / `conversations::delete`),
so no `[negative-perm]` spec is required (A10 N/A).

## Unit

- **TEST-1** (tier: unit) [covers: ITEM-1] file: `src-app/ui/src/modules/chat/core/utils/estimateConversationHeight.test.ts` — asserts: estimator is total (undefined/empty → floor), returns a LARGER estimate for a long (2-line-wrapping) title than a short one at the same width, never DECREASES when `message_count > 0` (the meta chip reserves horizontal space, monotonic non-decreasing), is width-sensitive (narrower width → taller estimate for the same title), and is memoized per (conversation, width bucket) (repeat call === same value, no throw).

- **TEST-2** (tier: unit) [covers: ITEM-2] file: `src-app/ui/src/modules/chat/core/utils/measuredHeightCache.test.ts` — asserts: (added cases to the existing suite) `setMeasuredHeight`/`getMeasuredHeight` round-trip conversation-UUID keys at a width bucket; `buildInitialMeasurementsCache(convIds, width)` emits one seed entry per id that has a cached height and OMITS uncached ids; a different width bucket MISSES stale-width heights — proving the id-generic cache is safely reused for conversation rows (DEC-2) with the message path unaffected.

- **TEST-3** (tier: unit) [covers: ITEM-6] file: `src-app/ui/src/modules/chat/core/utils/chatListMetrics.test.ts` — asserts: the pure `makeChatListMetrics` factory builds a LIVE metrics view — `corrections` reads through a mutable counter (not a snapshot), `reset()` zeroes the counter, and `totalSize()` reads through its getter. (Pure-logic complement; the DEV-gating + the RUNNING settle-to-~0 proof is TEST-6.)

## E2E (behavioural — RUNS each claim, B7)

- **TEST-4** (tier: e2e) [covers: ITEM-3, ITEM-4, ITEM-7] file: `src-app/ui/tests/e2e/visual/conversation-list-virtualization.spec.ts` — asserts: on the backend-free `?surface=seeded-conversation-list-long` gallery (≈200 seeded conversations driving the REAL `VirtualizedConversationList`), the mounted `chat-conversation-card-*` count is a bounded WINDOW **far less than 200** (e.g. < 40) while the "Showing 200 of 200" footer confirms the full set is loaded — i.e. only the visible window is in the DOM, not all rows.

- **TEST-5** (tier: e2e) [covers: ITEM-3, ITEM-4] file: `src-app/ui/tests/e2e/visual/conversation-list-virtualization.spec.ts` — asserts: scrolling the `chat-conversation-list-scroll` viewport UPDATES the window — a card mounted at the top detaches after scrolling down, and a deep card (near the end) that was NOT in the DOM becomes attached; scrolling back re-mounts the top card. Proves the window follows the scroll position.

- **TEST-6** (tier: e2e) [covers: ITEM-1, ITEM-6] file: `src-app/ui/tests/e2e/visual/conversation-list-virtualization.spec.ts` — asserts: **no row-height jank** — after resetting `window.__CHATLIST_METRICS__`, scrolling from top to a deep offset and pausing, the `corrections` counter settles to ~0 (≤ a small threshold) and `getTotalSize()` is stable across the pause (the estimate is close enough that measured rows don't keep re-correcting the scroll geometry / thumb). Zero console errors / page errors across the interaction.

- **TEST-7** (tier: e2e) [covers: ITEM-5] file: `src-app/ui/tests/e2e/chat/conversation-list-virtualization.spec.ts` — asserts: the virtualized list preserves existing behaviour — clicking a windowed card navigates to `/chat/{id}`; hover-reveal delete + selection checkbox still work on a mounted row; a card scrolled OUT and back retains correct content (stable-key measurement, no blank/pop-in row). Proves the surgical swap didn't regress per-row interactions.

- **TEST-8** (tier: e2e) [covers: ITEM-5] file: `src-app/ui/tests/e2e/chat/conversation-list-virtualization.spec.ts` — asserts: (REAL production path) seed ~120 conversations via `POST /api/conversations`, `goto('/chats')`, Load-More until "Showing 120 of 120", then assert the DOM `chat-conversation-card-*` count is still a bounded window (far < 120) — virtualization holds on the real page, and Load-More paging composes with it (composability with paged data). Mirrors `conversation-list-load-more.spec.ts` seeding.

## Regression guard (existing specs must still pass — enumerated so A5 tracks them)

- **TEST-9** (tier: e2e) [covers: ITEM-5] file: `src-app/ui/tests/e2e/chat/conversation-list-load-more.spec.ts` — asserts: (UNCHANGED existing spec, re-run) Load-More still fetches the next page and updates "Showing N of M" on the virtualized list.
- **TEST-10** (tier: e2e) [covers: ITEM-5] file: `src-app/ui/tests/e2e/chat/conversation-list-search.spec.ts` — asserts: (UNCHANGED existing spec, re-run) server-side search still filters and renders results / the no-match empty state through the virtualized path.

## Static gate (Phase 8)

- **TEST-11** (tier: unit) [covers: ITEM-7] file: `src-app/ui/src/dev/gallery/seededSurfaces.tsx` — asserts: (via `npm run check`: `check:gallery-coverage` + `check:state-matrix` + `check:testid-registry`) the new `seeded-conversation-list-long` surface (+ narrow 390px variant) is registered with the required coverage/state entries and the new `chat-conversation-list-scroll` testid is in the registry, so `npm run check (ui)` is green. `gate:ui` runtime-health reports zero HIGH findings on the populated + narrow surfaces (A7 boot/runtime canary).
