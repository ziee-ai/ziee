# PLAN_AUDIT — lazy-load-conversation-messages

Audit of PLAN.md against the actual codebase (worktree off origin/main @ 7f1ef2d3).

## Breakage risk

- **The dual-use `get_conversation_history` repo fn is the #1 breakage trap.**
  `repository/messages.rs::get_conversation_history` is called by
  `summarization/engine/summarizer.rs`, `summarization/chat_extension`,
  `memory/chat_extension`, `mcp/chat_extension`, `chat/extensions/title`, and
  `chat/core/services/streaming.rs` — ALL of which need the FULL branch history
  to build the LLM context. The plan (ITEM-1) adds a SEPARATE
  `list_message_window` / `get_message_window` and leaves the full-load fn
  untouched, so the AI-context path is unaffected. ✔ Verified via
  `grep get_conversation_history src/` (7 non-test callers, all context builders).
- **HTTP response shape change is breaking but internally contained.** The
  endpoint `GET /conversations/{id}/messages` changes from a bare
  `Vec<MessageWithContent>` to `PaginatedMessages`. The ONLY consumer is
  `Chat.store.ts::loadMessages` (`ApiClient.Message.getHistory`), which ITEM-6
  reworks in lockstep. `tests/chat/messages_test.rs` deserializes the response as
  `Vec<serde_json::Value>` — those assertions (ITEM-3 tests) must be updated to
  read `.messages`. No other frontend/integration caller of `getHistory` exists
  (grep confirms). ✔
- **Junction-clone `created_at` inheritance is load-bearing for the cursor.**
  `create_branch_from_message` / `edit_message` clone `branch_messages` rows
  COPYING the parent's `created_at` (`SELECT ... created_at ... WHERE created_at <
  $cutoff`). So within any one branch, `created_at` values are the original
  insertion times and the ordering is stable and consistent with the existing
  `ORDER BY bm.created_at ASC`. The `(created_at, message_id)` composite cursor is
  a strict refinement of the current ordering → no visible reorder, and ties
  (should any exist) break deterministically. ✔
- **Scroll-anchor viewport access.** `DivScrollY` forwards its ref to
  `OverlayScrollbarsComponent`; the real scrollable element is
  `ref.osInstance()?.elements().viewport`. On mobile `nativeFlow` mode it renders
  a plain div (window scroll) with NO OverlayScrollbars instance — ITEM-9 must
  guard the viewport lookup and fall back to `document.scrollingElement` (or skip
  restore) in native mode. Flagged in DEC-2/DEC-7; not a blocker.

## Pattern conformance

- ITEM-1 mirrors `messages.rs` `query_as!` + `INNER JOIN branch_messages` +
  `get_message_contents_batch` (the exact idiom of the existing
  `get_conversation_history`). ✔
- ITEM-2/3 mirror `mcp/tool_calls` `ListToolCallsQuery` (Query + `#[serde(default)]`)
  and the `McpToolCallListResponse` envelope, and the sibling
  `get_conversation_history_docs` for the aide `_docs` shape. ✔
- ITEM-4 mirrors `repository/core.rs::get_conversation_history` facade. ✔
- ITEM-6/7/11 mirror existing `Chat.store.ts` Map-rebuild + `defineStore`
  idioms; ITEM-8 mirrors `branchAnchor.utils.ts` + `findMatches.test.ts`. ✔
- ITEM-9 mirrors the IntersectionObserver + `useLayoutEffect` already in
  `ConversationPage.tsx` (bottom sentinel / initial-jump) and the
  `[data-message-id]` + `scrollIntoView` jump already in `ConversationFindBar`. ✔

## Migration collisions

- **None.** No new migration. The keyset query relies on the EXISTING index
  `idx_branch_messages_branch_id ON branch_messages(branch_id, created_at)`
  (migration 13), which is exactly the access path the window query needs. Latest
  migration on disk is `132`; this feature adds `0`. ✔ (No schema change → no
  `cargo clean` / build-DB reset needed.)

## Search-endpoint audit (ITEM-12/13)

- **F3 find is client-side today and lazy-load breaks it.** `findMatches.ts` runs
  over `Stores.Chat.messages` (the loaded window only); `ConversationFindBar`
  scrolls a loaded `[data-message-id]` into view. Under lazy-load the loaded set
  is a slice, so matches in unloaded messages become invisible — a regression the
  server-side search (ITEM-12) fixes. ✔ Verified: `findMatches` imports nothing
  server-side; `conversation-find.spec.ts` asserts client-only behavior (that
  spec must be updated to the server-backed flow).
- **A per-conversation message-search endpoint does NOT exist yet.** The only
  message-text search is embedded inside `conversations.rs::list_conversations`
  (across conversations, active-branch EXISTS-join). ITEM-12 is genuinely new but
  reuses that exact join predicate scoped to one conversation's active branch —
  no new index needed (`message_contents` text match + `branch_messages` on
  `branch_id`; the `idx_branch_messages_branch_id` + message_contents PK cover
  it; substring ILIKE is a scan bounded by the branch size, acceptable for an
  interactive per-conversation search, matching the existing list-search cost
  profile). ✔
- **Branch scoping is a correctness + isolation requirement.** Search MUST be
  scoped to the conversation's `active_branch_id` (like the list search comment
  notes) so a superseded edit-branch's content isn't surfaced, and so results are
  jump-to-able via `around=` (which is active-branch-scoped). Cross-user leakage
  is prevented by the same `get_conversation(id, user.id)` ownership check the
  history handler already does. ✔
- **Result → around jump is already-built plumbing.** ITEM-13 selecting a match
  reuses ITEM-7 `jumpToMessage` (around=) + ITEM-9 scroll/highlight; "load more
  around" is the ITEM-9 before / ITEM-7 after infinite-scroll continuing from the
  jumped position. No new scroll machinery. ✔

## OpenAPI regen

- **Required (ITEM-5).** ITEM-2 adds a new request query type
  (`MessageHistoryQuery`) and a new response type (`PaginatedMessages`) on the
  `Message.getHistory` operation (plus ITEM-12's `Message.searchInConversation` op
  + `MessageSearchQuery`/`MessageSearchResults`/`MessageSearchMatch` types) →
  `just openapi-regen` must run for BOTH
  binaries; `npm run check` must pass in BOTH `ui` and `desktop/ui`
  ([[project_openapi_regen_both_binaries]]). The generated `openapi.json` /
  `api-client/types.ts` are excluded from the phase-6 coverage law and phase-3/8
  frontend gates (mechanically generated). Golden parity test
  `openapi::emit_ts::tests::types_ts_parity` will fail if regen is skipped —
  caught in phase 8. ✔

## Per-item verdicts

- **ITEM-1** — verdict: PASS — new windowed repo fn beside the untouched full-load; reuses existing index (migration 13) + `get_message_contents_batch`.
- **ITEM-2** — verdict: CONCERN — new request/response types require `just openapi-regen` in both binaries (tracked as ITEM-5); envelope shape (`has_more_before`/`has_more_after` vs a single `next_cursor`) is a DEC-1 decision pending user ack.
- **ITEM-3** — verdict: CONCERN — breaking response-shape change to an existing endpoint; sole consumer (Chat.store) reworked in ITEM-6 and the `messages_test.rs` assertions updated in phase-3 tests. Contained, not blocked.
- **ITEM-4** — verdict: PASS — thin facade mirroring the existing `core.rs` delegate.
- **ITEM-5** — verdict: PASS — mechanical regen; golden parity test backstops it.
- **ITEM-6** — verdict: CONCERN — the SSE `complete`/`reloadOpen` path currently does a FULL `loadMessages` replace; switching to merge-tail must preserve the scrolled-up window AND still reconcile finalized ids. Behavior-critical; enumerated in TEST (SSE-append + scrolled-up-preserved). Not blocked.
- **ITEM-7** — verdict: PASS — additive jump/after loaders; around= is designed in now (not a retrofit) per the constraint.
- **ITEM-8** — verdict: PASS — pure util mirroring `branchAnchor.utils.ts`, unit-tested like `findMatches.test.ts`.
- **ITEM-9** — verdict: CONCERN — the hardest UX-correctness surface (async-height anchor drift + OverlayScrollbars viewport access + native/mobile fallback). Mitigation (anchor-element + useLayoutEffect + short-lived ResizeObserver + `overflow-anchor:none`) is DEC-2, pending user ack. Native-mode fallback flagged above. Not blocked.
- **ITEM-10** — verdict: CONCERN — new render state ("loading older") needs a gallery cell or `check:state-matrix` (inside `npm run check`) fails phase 8. Budgeted in the item. Not blocked.
- **ITEM-11** — verdict: PASS — extends the existing A→B-switch `messages: new Map()` reset to the new window fields; branch cursors are correctly scoped to the active branch path.
- **ITEM-12** — verdict: PASS — new endpoint reusing the proven active-branch text-match join from `conversations.rs`; owner-scoped via the existing conversation ownership check; no new index/migration. New types require regen (ITEM-5).
- **ITEM-13** — verdict: CONCERN — replaces the client-only F3 find with server-backed search; `conversation-find.spec.ts` must be updated from client-only to the server-backed flow, and a debounce is needed so keystrokes don't storm the endpoint. Result→jump reuses existing plumbing (ITEM-7/9). New results-list render state needs a gallery cell (`check:state-matrix`). Not blocked.
