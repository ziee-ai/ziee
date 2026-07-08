# TESTS — lazy-load-conversation-messages

Every ITEM is covered by ≥1 TEST. Backend repo/endpoint correctness is
integration (needs Postgres); pure validation + pure frontend math are unit;
the user-visible reverse-infinite-scroll + jump flows are e2e. No cosmetic
tests — only the external boundary (HTTP) is crossed; scroll anchoring is
asserted against real rendered geometry.

## Unit

- **TEST-1** (tier: unit) [covers: ITEM-2] file: `src-app/server/src/modules/chat/core/types/message.rs` — asserts: `MessageHistoryQuery` limit clamps to 1..=100 (default 30), and the cursor-mutual-exclusion helper rejects any request with ≥2 of {before, after, around} set (→ the 400 path).
- **TEST-2** (tier: unit) [covers: ITEM-8] file: `src-app/ui/src/modules/chat/core/utils/scrollAnchor.utils.test.ts` — asserts: `computeScrollRestore` returns the exact scrollTop delta that re-pins the captured anchor when the container grows above it (prepend), returns 0 when nothing changed, and `captureTopAnchor` picks the top-most visible `[data-message-id]` (fed synthetic rects).
- **TEST-3** (tier: unit) [covers: ITEM-6, ITEM-7] file: `src-app/ui/src/modules/chat/core/stores/messageWindow.test.ts` — asserts: the pure window-merge helper (`prependWindow` / `mergeTailWindow` extracted from the store) rebuilds the ordered Map oldest→newest on prepend, upserts the tail without dropping scrolled-up older entries on merge, and de-dups overlapping ids — proving ITEM-6's merge-tail and ITEM-7's around/after reconciliation keep render order.
- **TEST-13** (tier: unit) [covers: ITEM-5] file: `src-app/server/src/openapi/emit_ts.rs` — asserts: the golden parity test `openapi::emit_ts::tests::types_ts_parity` regenerates `types.ts` from the committed `openapi.json` and byte-matches the committed `types.ts` — i.e. it FAILS if `MessageHistoryQuery`/`PaginatedMessages` were added without running `just openapi-regen`, backstopping ITEM-5 in both binaries.

## Integration

- **TEST-4** (tier: integration) [covers: ITEM-1, ITEM-3, ITEM-4] file: `src-app/server/tests/chat/messages_test.rs` — asserts: TAIL load (no params) on a 25-message conversation returns the newest `limit` messages ASC with `has_more_before=true`/`has_more_after=false`; `before=<oldest returned id>` returns the next-older page ASC with correct `has_more_before`; walking `before` to the top yields `has_more_before=false` and never repeats/skips a message (full set reconstructed exactly once).
- **TEST-5** (tier: integration) [covers: ITEM-1, ITEM-7] file: `src-app/server/tests/chat/messages_test.rs` — asserts: `around=<middle id>` returns a window CENTERED on that message (≈half older + the target + ≈half newer) with BOTH `has_more_before` and `has_more_after` true on a long conversation; `after=<newest-in-window id>` then loads the next-newer page. Confirms the bidirectional (around/after) design works, not just before.
- **TEST-6** (tier: integration) [covers: ITEM-3] file: `src-app/server/tests/chat/messages_test.rs` — asserts: validation + errors — empty conversation returns `{messages:[], has_more_before:false, has_more_after:false}`; unknown `before` id → 404; two cursors set (`before` + `around`) → 400; a `before` id that belongs to a DIFFERENT branch than the active one → 404 (cursor scoped to active branch).
- **TEST-7** (tier: integration) [covers: ITEM-11] file: `src-app/server/tests/chat/branches_test.rs` — asserts: after `activate` of a sibling branch, a TAIL load returns THAT branch's path tail (not the previous branch's), and an `around` cursor from the old branch → 404 against the newly-active branch — proving pagination follows the active branch path.
- **TEST-8** (tier: integration) [covers: ITEM-1] file: `src-app/server/tests/chat/messages_test.rs` — asserts: content blocks are batch-loaded and correctly associated per message in a window (a windowed message with multiple content blocks returns them all, in `sequence_order`), mirroring the full-load's batch guarantee — proving the window reuses `get_message_contents_batch` without N+1 or cross-message bleed.

## E2E

- **TEST-9** (tier: e2e) [covers: ITEM-6, ITEM-9, ITEM-10] file: `src-app/ui/tests/e2e/chat/lazy-load-messages.spec.ts` — asserts: opening a long seeded conversation loads only the recent page (older message text NOT in DOM initially, top spinner/`hasMoreBefore` present); scrolling to the top prepends older messages AND the previously-top-visible message stays put (scroll position does not teleport — measured `data-message-id` bounding box delta ≈ 0 across the prepend).
- **TEST-10** (tier: e2e) [covers: ITEM-7, ITEM-9] file: `src-app/ui/tests/e2e/chat/lazy-load-jump-to-message.spec.ts` — asserts: navigating with a `#message-<id>` deep-link to an initially-UNLOADED message loads the around-window, scrolls it into CENTER view, and applies the highlight ring; scrolling down from there loads newer messages (after=).
- **TEST-11** (tier: e2e) [covers: ITEM-11] file: `src-app/ui/tests/e2e/chat/lazy-load-branch-reset.spec.ts` — asserts: after scrolling up + loading older pages, switching branches (BranchNavigator) RESETS the window to the new branch's tail (old older-pages gone, new branch tail shown), and re-scrolling up paginates the new branch.
- **TEST-12** (tier: e2e) [covers: ITEM-6, ITEM-10] file: `src-app/ui/tests/e2e/chat/lazy-load-sse-and-short.spec.ts` — asserts: (a) sending a new message while scrolled to bottom APPENDS the user+assistant turn at the bottom (SSE path) without discarding loaded older pages; (b) a SHORT conversation (< initial page size) shows ALL messages with no top spinner and never triggers a pagination fetch (network assertion: no `before=` request fired).

## Gate note

The diff touches `src-app/ui/**` (store, page, MessageList, utils, gallery), so
the phase-3 gate requires ≥1 `tier: e2e` test — satisfied by TEST-9..TEST-12.
Backend-only generated `openapi.json`/`api-client/types.ts` are excluded from
the UI-work determination.
