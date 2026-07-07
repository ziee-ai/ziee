# PLAN — chat-power-features (F3)

Scope: only the **confirmed-missing** portions of clusters 2a/2b/2c from
`CAPABILITY_AUDITED.md`. Items already shipping (export, bulk-delete, resize,
auto-scroll-when-at-bottom, keyboard shortcuts) are OUT of scope and NOT rebuilt.

## Items

- **ITEM-1**: Search-within-open-conversation. A find bar in `ConversationPage`
  (toggled by a header button and Cmd/Ctrl-F while the conversation is focused)
  that matches the query against the text of loaded messages, shows "match X of
  Y", and Enter / arrows / prev-next buttons jump to and scroll each matched
  message into view with a transient highlight. Client-side over the already-
  loaded `Stores.Chat.messages` (no backend). Esc closes and clears.
- **ITEM-2**: Scroll-to-bottom / jump-to-latest button. A floating button pinned
  to the bottom-right of the message scroll area that appears ONLY when the user
  is scrolled up (not at bottom) and, on click, scrolls to the existing
  `messagesEndRef` sentinel. Reuses the existing `isAtBottomRef`
  IntersectionObserver in `ConversationPage`; the observer's boolean is surfaced
  to render via state.
- **ITEM-3**: Collapse long messages by default. A generic height-clamp wrapper
  (`CollapsibleBlock`) that clamps content taller than a threshold, fades the
  bottom, and shows a "Show more / Show less" toggle. Applied to long
  assistant/user text bubbles in `ChatMessage` (the confirmed gap — `ChatMessage`
  has no collapse/expand). NOT applied to the MCP tool-result card, which
  ALREADY collapses by default (`isExpanded` gate) with a `max-h-40` result body
  — re-clamping it is out of scope. Never clamps the actively-streaming message
  (so live output isn't hidden mid-stream).
- **ITEM-4**: Backend — search message CONTENT. Extend the `GET
  /api/conversations` list endpoint + repository with an optional `search`
  query param; when present, filter to conversations whose title OR any `text`
  message-content block matches (case-insensitive substring, `ILIKE`), and make
  `count_conversations` honor the same predicate so pagination totals are
  correct.
- **ITEM-5**: Backend — sort conversations. Extend the same list endpoint +
  repository with an optional `sort` query param
  (`recent` | `oldest` | `alpha` | `most_messages`; default `recent` =
  current `updated_at DESC` behavior), applied as a whitelisted `ORDER BY`.
- **ITEM-6**: Frontend — wire content-search + sort into history. Route the
  `ChatHistory` store's search through the backend `search` param (replacing the
  client-side title-only `filteredConversations` filter) and add a sort control
  in `ChatHistoryPage`/`ConversationList` bound to a new `sort` store field.
  Search + sort + pagination all resolve server-side via `loadConversations`.
- **ITEM-7**: Drafts persist across navigation. Persist the composer's unsent
  text per-conversation (and a `new` bucket for the new-chat page) to
  `localStorage`; restore it into the textarea when the composer mounts for that
  conversation, save on input (debounced), and clear it on successful send.
  Implemented in the text extension (`TextInput` + a small `chatDrafts` helper +
  `Text.store`), not in chat core.
- **ITEM-8**: Paste image from clipboard. An `onPaste` handler on the composer
  that extracts image blobs from `clipboardData` and routes them through the
  existing `Stores.File.uploadFiles(...)` attachment path. Implemented in the
  file chat-extension as a slot-mounted `FilePasteHandler` mirroring
  `FileUploadArea`'s composer-host discovery, so chat stays file-agnostic.

## Files to touch

Frontend (src-app/ui):
- `src/modules/chat/pages/ConversationPage.tsx` — find bar host + jump button
  wiring (ITEM-1, ITEM-2)
- `src/modules/chat/components/ConversationFindBar.tsx` — NEW find bar (ITEM-1)
- `src/modules/chat/components/JumpToLatestButton.tsx` — NEW scroll-to-bottom
  button (ITEM-2)
- `src/modules/chat/components/MessageList.tsx` — pass find-match context /
  highlight target (ITEM-1)
- `src/modules/chat/components/ChatMessage.tsx` — apply `CollapsibleBlock` to
  long text bubbles; expose a highlight/scroll target for find (ITEM-1, ITEM-3)
- `src/modules/chat/components/CollapsibleBlock.tsx` — NEW clamp/expand wrapper
  (ITEM-3). Chat-local (single consumer); promote to kit only if reused.
- `src/modules/chat/stores/ChatHistory.store.ts` — `sort` state, backend-routed
  search, remove client title filter (ITEM-5, ITEM-6)
- `src/modules/chat/pages/ChatHistoryPage.tsx` — sort control placement (ITEM-6)
- `src/modules/chat/components/ConversationList.tsx` — sort control + drop
  `filteredConversations` usage (ITEM-6)
- `src/modules/chat/extensions/text/components/TextInput.tsx` — draft restore /
  save (ITEM-7)
- `src/modules/chat/extensions/text/chatDrafts.ts` — NEW localStorage draft
  helper (ITEM-7)
- `src/modules/chat/extensions/text/Text.store.ts` — register `clearDraft`
  (ITEM-7)
- `src/modules/chat/extensions/text/extension.tsx` — clear draft in
  `onMessageSent` (ITEM-7)
- `src/modules/file/chat-extension/components/FilePasteHandler.tsx` — NEW paste
  handler (ITEM-8)
- `src/modules/file/chat-extension/extension.tsx` — register the paste handler
  slot (ITEM-8)
- gallery fixtures/entries as needed for any new render state (ITEM-1..3)
- generated `src/api-client/types.ts` (regen only — ITEM-4/5)

Backend (src-app/server):
- `src/modules/chat/core/handlers/conversations.rs` — `search` + `sort` on
  `PaginationQuery`; pass through (ITEM-4, ITEM-5)
- `src/modules/chat/core/repository/conversations.rs` — filtered/sorted
  `list_conversations` + filtered `count_conversations` (ITEM-4, ITEM-5)
- `src/modules/chat/core/repository/core.rs` — signature passthrough (ITEM-4,
  ITEM-5)
- `openapi/openapi.json` (regen), desktop mirror (regen)

## Patterns to follow

- **Backend list search/sort** → mirror the existing `list_conversations` /
  `count_conversations` in `chat/core/repository/conversations.rs` and the
  `PaginationQuery`/`with_permission` handler idiom in
  `chat/core/handlers/conversations.rs`. Whitelist `sort` to a fixed enum (never
  interpolate) — same defensive style as the existing parameterized queries.
- **Text JSONB match** → text lives as `content->>'text'` on
  `message_contents WHERE content_type='text'` (see the insert at
  `repository/messages.rs:346`). Match with `EXISTS (...)` + `ILIKE`.
- **Store search/sort wiring** → mirror the existing `ChatHistory.store.ts`
  `loadConversations`/`setSearchQuery` and its `defineStore` shape; keep the
  500ms debounce already in `ConversationList`.
- **Find bar / floating button** → build from kit primitives
  (`Input`, `Button`, `Tooltip`) exactly as `ConversationList`'s search box and
  the composer buttons do; use semantic tokens per `DESIGN_SYSTEM.md`.
- **Paste handler** → mirror `FileUploadArea.tsx` verbatim: a hidden sentinel
  `span`, `closest('[data-chat-composer]')` host discovery, DOM listener added
  in `useEffect`, `Stores.File.__state.uploadFiles(...)` for the upload. Same
  slot-registration idiom in `file/chat-extension/extension.tsx`.
- **Draft persistence** → mirror the getter/setter/clearer registration already
  in `Text.store.ts` + `TextInput.tsx`; add `clearDraft` the same way and clear
  it from the text extension's existing `onMessageSent` hook.
- **Collapsible** → reuse an existing shadcn/kit collapsible/height-clamp if one
  exists (discover first); otherwise a minimal token-styled wrapper matching the
  MCP resource-links collapse affordance already in the chat surface.
- **E2E** → mirror `tests/e2e/chat/conversation-list-search.spec.ts` and
  `chat-basic.spec.ts` (seed via REST, `loginAsAdmin`, semantic selectors).
