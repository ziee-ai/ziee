# PLAN — lazy-load-conversation-messages

Lazy-load conversation messages: load the most-recent page first, fetch older
messages on scroll-up (reverse infinite scroll), and support jump-to a possibly
-unloaded message (citations / deep-links / future server-side search). ziee
conversations are **branching trees** — pagination walks the ACTIVE BRANCH PATH
via keyset/cursor (never LIMIT/OFFSET). Virtualization is explicitly DEFERRED
(variable-height tool-result cards) — noted as a follow-up.

## Items

- **ITEM-1**: Add a keyset (cursor) windowed message loader in `repository/messages.rs` — `list_message_window(branch_id, mode, limit)` where `mode ∈ {Tail, Before(id), After(id), Around(id)}`, plus a contents-batched `get_message_window` wrapper. Walks the active branch path via the junction table, ordered by the composite key `(branch_messages.created_at, message_id)` for total-order stability, using the fetch-`limit+1` technique to compute `has_more_before` / `has_more_after`. Leaves the existing full-load `get_conversation_history` (used by summarization/memory/mcp/title/streaming for AI context) **untouched**.
- **ITEM-2**: Add the response envelope `PaginatedMessages { messages: Vec<MessageWithContent> (chronological ASC), has_more_before: bool, has_more_after: bool }` and the query type `MessageHistoryQuery { before?, after?, around?: Uuid, limit?: i64 }` (with `#[serde(default)]`), in `core/types/message.rs`. Cursor = a `message_id` resolved server-side to its junction-row `created_at` in the active branch (message_id IS the cursor; the window endpoints `messages[0].id` / `messages[last].id` are the next before/after cursors — no opaque blob).
- **ITEM-3**: Rework the HTTP handler `get_conversation_history` (`core/handlers/messages.rs`) to take `Query<MessageHistoryQuery>`, resolve the conversation's active branch (as today), dispatch to the windowed repo via a new facade method, and return `Json<PaginatedMessages>`. Update `get_conversation_history_docs` (new params + 200 envelope + 400 + 404). Unknown / not-in-active-branch cursor id → 404; ≥2 of before/after/around set → 400.
- **ITEM-4**: Add the facade method `Repos.chat.core.get_message_window(...)` in `repository/core.rs` delegating to ITEM-1, mirroring the existing `get_conversation_history` facade. Limit clamp (default 30, 1..=100) + cursor-mutual-exclusion validation live in the handler/query type.
- **ITEM-5**: Regenerate OpenAPI + TS types for BOTH binaries (`just openapi-regen`): server → `ui/`, desktop → `desktop/ui/`. New `MessageHistoryQuery` params on `Message.getHistory` + new `PaginatedMessages` response type in both `api-client/types.ts`.
- **ITEM-6**: `Chat.store.ts` windowed message state + loaders: add `hasMoreBefore`, `hasMoreAfter`, `loadingOlder`, `oldestLoadedId`, `newestLoadedId`; rework `loadMessages(id)` to load the TAIL page and read the envelope; add `loadOlderMessages()` (before=oldest) that PREPENDS via an ordered-Map rebuild (insertion order is the render order in `MessageList`). Change SSE `complete` / `reloadOpen` to MERGE the tail page (upsert) into the existing window instead of replacing the whole Map (so a user who scrolled up + loaded older pages keeps them; new turns still append at the bottom).
- **ITEM-7**: `Chat.store.ts` jump-to-message: `jumpToMessage(messageId)` (around=) replaces the window with a centered window + sets flags; `loadNewerMessages()` (after=newest) for scrolling DOWN after a mid-conversation jump. Both reconcile ordered-Map + `has_more_*`.
- **ITEM-8**: Pure scroll-anchor utilities `core/utils/scrollAnchor.utils.ts`: `captureTopAnchor(viewport, container)` → `{ anchorId, savedTop } | null` (top-most visible `[data-message-id]`), and `computeScrollRestore(viewport, container, anchor)` → the `scrollTop` delta to re-pin it. Pure/DOM-measuring but side-effect-free (returns numbers) so the math is unit-testable.
- **ITEM-9**: `ConversationPage.tsx` reverse-infinite-scroll wiring: a TOP sentinel + `IntersectionObserver` on the DivScrollY viewport (prefetch threshold via `rootMargin`) that, guarded by `hasMoreBefore && !loadingOlder`, captures the anchor, dispatches `loadOlderMessages`, and restores scroll in `useLayoutEffect` (before paint) — reinforced by a short-lived `ResizeObserver` so late async height (images/katex/mermaid/shiki in the prepended block) doesn't shift the anchor. Set `overflow-anchor: none` on the scroll content to stop the browser's own anchoring from fighting the manual restore. Add a `#message-<id>` hash deep-link handler → `jumpToMessage` → center-scroll + highlight (reuses the F3 highlight ring).
- **ITEM-10**: `MessageList.tsx` top affordance: a top-loading spinner row shown while `loadingOlder`, and nothing when `!hasMoreBefore` (short/loaded-to-top conversation never paginates). Add the gallery state cell for the new "loading older" render state (satisfies `check:state-matrix`).
- **ITEM-11**: Branch correctness: `activateBranch` and the `branchChangedDuringStream` reconcile RESET the window to the new active branch's TAIL (cursors are only valid within the active branch path); confirm `loadMessages` clears `oldestLoadedId`/`has_more_*` on every full (non-prepend) load. New-conversation / A→B switch already clears `messages` — extend that reset to the new window fields.

## Files to touch

Backend:
- `src-app/server/src/modules/chat/core/repository/messages.rs` (ITEM-1 — new windowed fns; existing full-load untouched)
- `src-app/server/src/modules/chat/core/repository/core.rs` (ITEM-4 — facade method)
- `src-app/server/src/modules/chat/core/types/message.rs` (ITEM-2 — envelope + query type + `#[cfg(test)]` validation)
- `src-app/server/src/modules/chat/core/handlers/messages.rs` (ITEM-3 — handler + docs)
- `src-app/server/openapi/openapi.json` (ITEM-5 — generated)
- `src-app/server/src/api-client/types.ts` — N/A (server has no client); the UI/desktop clients below are the generated artifacts

Generated (ITEM-5, do not hand-edit):
- `src-app/ui/openapi/openapi.json`, `src-app/ui/src/api-client/types.ts`
- `src-app/desktop/ui/openapi/openapi.json`, `src-app/desktop/ui/src/api-client/types.ts`

Frontend:
- `src-app/ui/src/modules/chat/core/stores/Chat.store.ts` (ITEM-6, ITEM-7, ITEM-11)
- `src-app/ui/src/modules/chat/core/utils/scrollAnchor.utils.ts` (ITEM-8 — new)
- `src-app/ui/src/modules/chat/pages/ConversationPage.tsx` (ITEM-9, ITEM-11)
- `src-app/ui/src/modules/chat/components/MessageList.tsx` (ITEM-10)
- `src-app/ui/src/dev/gallery/**` — MessageList "loading older" state cell (ITEM-10)
- Mirror any of the above that the desktop UI copies (desktop reuses `src-app/ui/src` for chat — verify no desktop-local fork needed).

## Patterns to follow

- **Keyset window repo fn** (ITEM-1): mirror the existing `messages.rs` query style (`query_as!` + the `branch_messages` INNER JOIN, ordered by `bm.created_at`) and reuse `get_message_contents_batch` exactly as `get_conversation_history` does. The `(created_at, message_id)` composite-cursor + fetch-`limit+1` idiom is standard keyset; no existing keyset reference in-repo (existing pagination is offset-style), so keep the SQL self-documenting.
- **Query params + paginated response** (ITEM-2/3): mirror `mcp/tool_calls/handlers.rs::ListToolCallsQuery` (`#[derive(Deserialize, JsonSchema)]` + `#[serde(default)]` + `Query(params)`) and `mcp/tool_calls/models.rs::McpToolCallListResponse` (a plain `#[derive(Serialize, JsonSchema)]` envelope struct). Handler + `_docs` shape mirrors the existing `get_conversation_history` / `get_conversation_history_docs` in the same file.
- **Facade method** (ITEM-4): mirror `repository/core.rs::get_conversation_history` (thin delegate to the `messages::` free fn).
- **Store windowing** (ITEM-6/7/11): mirror the existing `Chat.store.ts` `loadMessages` / `activateBranch` / `applyStreamFrame` idioms (Map rebuild via `new Map(...)`, `defineStore` `set`/`get`, `Stores.Chat.$.field` reads in handlers). Ordered-Map rebuild follows the same `new Map(array.map(...))` construction already used in `loadMessages`.
- **Pure util + unit test** (ITEM-8): mirror `core/utils/branchAnchor.utils.ts` (pure, side-effect-free helpers) and `components/findMatches.test.ts` (node `--test` unit spec via `test:unit`).
- **Reverse-infinite-scroll + observers** (ITEM-9): mirror the existing `ConversationPage.tsx` IntersectionObserver + `useLayoutEffect` scroll idiom already used for the bottom sentinel / initial-jump; thread the viewport element from `DivScrollY`'s forwarded `OverlayScrollbarsComponentRef` (`.osInstance().elements().viewport`).
- **Find/highlight reuse** (ITEM-9 jump): reuse `ConversationFindContext` + the `[data-message-id]` attribute and `scrollIntoView({ block: 'center' })` already used by `ConversationFindBar`.
- **Gallery state cell** (ITEM-10): mirror existing MessageList/chat gallery entries under `src/dev/gallery/`.
