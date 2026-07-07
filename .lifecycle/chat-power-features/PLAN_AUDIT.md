# PLAN_AUDIT — chat-power-features

Audit of PLAN.md against the actual codebase (worktree off origin/main @ 786b2689).

## Breakage risk

- `list_conversations` / `count_conversations` (repository free fns + `core.rs`
  wrappers) have exactly ONE caller each: the `list_conversations` handler
  (`handlers/conversations.rs:120,135`). Adding `search`/`sort` params to the
  signatures breaks no other caller. Verified via grep — no projects/other
  module calls these two fns (projects use their own
  `/projects/{id}/conversations` path).
- The `ChatHistory` store's `loadConversations` is also invoked by
  `RecentConversationsWidget` and `NewChatPage` (sidebar eager-load). Routing
  search/sort through it means those callers pass defaults (no search, `recent`)
  — safe. RISK: a lingering non-empty `searchQuery`/non-default `sort` in store
  state would leak into the sidebar's `recentConversations`. MITIGATION (planned
  in ITEM-6): only refresh `recentConversations` on an UNFILTERED, default-sort,
  page-1 load; the sidebar keeps showing true most-recent.
- Removing client-side `filteredConversations` touches 10 references in
  `ChatHistory.store.ts` + 1 in `ConversationList.tsx`. All are mechanical
  (delete/bulkDelete/selectAll/updateTitle/sync now operate on
  `conversations` only). RISK: `selectAll` currently selects the filtered view;
  after the change the backend already returns only matches, so `conversations`
  IS the filtered view — semantics preserved.
- ITEM-2 surfaces the existing `isAtBottomRef` boolean to render state. The
  existing scroll effects read the ref, not state — adding a parallel `useState`
  updated inside the same IntersectionObserver callback does not disturb them.
- ITEM-3 clamp must exclude the streaming message; `ChatMessage` can read
  `Stores.Chat.isStreaming` + whether this message is the last/streaming one.
  RISK: clamping a message that is growing would hide new tokens — gated off for
  the streaming message (DEC-6).
- ITEM-7 draft restore must not clobber edit-prefill (`startEditMessage` calls
  `TextStore.setText`). MITIGATION (DEC-7): restore only on mount when the
  textarea is empty and no `editingMessage` is set; suppress save while editing.
- ITEM-8 paste handler attaches a `paste` listener to `[data-chat-composer]`.
  RISK: double-handling if the textarea also pastes text — images come via
  `clipboardData.files`/image items only; text paste is untouched (we don't
  `preventDefault` for non-image payloads).

## Pattern conformance

- Backend search/sort mirrors `list_conversations`/`count_conversations` exactly
  (same `sqlx::query!`, `ORDER BY`, `to_chrono_datetime` mapping). `sort` is a
  whitelisted match→`&str` `ORDER BY` fragment (never interpolated user text);
  `search` binds as a parameter used in `title ILIKE $n OR EXISTS(... ILIKE $n)`.
- `PaginationQuery` gets `search: Option<String>` + `sort: Option<String>` with
  `#[serde(default)]`, matching the existing `page`/`limit` defaulting idiom;
  `schemars::JsonSchema` derive already present → OpenAPI/TS params regenerate.
- Store/UI changes mirror the existing `defineStore` `ChatHistory` shape and the
  `ConversationList` search-box + debounce idiom. Sort control uses kit `Select`
  / `DropdownMenu` (discover the existing one) per `DESIGN_SYSTEM.md` tokens.
- Paste handler mirrors `FileUploadArea.tsx` (sentinel span,
  `closest('[data-chat-composer]')`, DOM listener, `uploadFiles`), registered in
  `file/chat-extension/extension.tsx` `slots` exactly like `FileUploadArea`.
- Draft persistence mirrors the `getMessage/setMessage/clearMessage` registration
  pattern in `Text.store.ts` + `TextInput.tsx`; `clearDraft` added the same way
  and called from the text extension's existing `onMessageSent`.

## Migration collisions

- NONE. No new migration. Latest migration is `00000000000131`; this feature adds
  no table/column/index. Content search uses `content->>'text'` + `ILIKE` against
  the existing `message_contents` table (the existing `idx_message_contents_type`
  on `content_type` helps the `content_type='text'` predicate). No `pg_trgm`/FTS
  index added — per-user data volume is bounded; a trigram index is a documented
  future optimization (DEC-3), not required for this scope.

## OpenAPI regen

- REQUIRED. Adding `search`/`sort` to `PaginationQuery` changes the
  `Conversation.list` endpoint params (`types.ts` line 6793 currently
  `{ limit?; page? }`). Must run `just openapi-regen` so BOTH `src-app/ui` and
  `src-app/desktop/ui` `api-client/types.ts` + both `openapi.json` regenerate,
  and verify `npm run check` (incl. the `types_ts_parity` golden test) in both
  workspaces. No response-body shape change (`ConversationListResponse`
  unchanged) — only query params grow.

## Per-item verdicts

- **ITEM-1** — verdict: PASS — client-side find over `Stores.Chat.messages`
  (fully loaded for the active branch); kit `Input`/`Button` primitives; no
  backend. Message-level match+scroll+highlight is the tractable, testable shape.
- **ITEM-2** — verdict: PASS — reuses the existing `isAtBottomRef` observer +
  `messagesEndRef` sentinel already in `ConversationPage`.
- **ITEM-3** — verdict: PASS — scoped to long text bubbles in `ChatMessage` (the
  confirmed gap); MCP tool-result card already collapses (`isExpanded`+`max-h-40`)
  so it is explicitly excluded — no redundant rebuild.
- **ITEM-4** — verdict: CONCERN — needs `just openapi-regen` (new `search` query
  param) + a filtered `count_conversations`. Resolved: regen in phase 8 static
  gate; count fn takes the same predicate. Not blocking.
- **ITEM-5** — verdict: CONCERN — needs `just openapi-regen` (new `sort` query
  param) + whitelisted ORDER BY. Resolved same as ITEM-4. Not blocking.
- **ITEM-6** — verdict: PASS — store refactor removes client title-filter and
  routes search/sort to backend; `recentConversations` guarded to unfiltered
  loads; mirrors existing store idioms.
- **ITEM-7** — verdict: PASS — localStorage draft via the existing register
  pattern; edit-prefill + streaming guarded per DEC-7.
- **ITEM-8** — verdict: PASS — mirrors `FileUploadArea` composer-host discovery
  and `uploadFiles` path; chat stays file-agnostic.
