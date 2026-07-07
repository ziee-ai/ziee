# DECISIONS — chat-power-features

All inputs the implementation needs, resolved up front. No open markers remain.

### DEC-1: How is message-content search matched — substring or full-text?
**Resolution:** Case-insensitive substring via Postgres `ILIKE '%term%'`,
matching `title` OR any `content_type='text'` block's `content->>'text'`.
**Basis:** convention — the existing title filter uses JS `title.includes()`
(case-insensitive) in `ChatHistory.store.ts:67`; `ILIKE` is its server-side
equivalent. Keeps scope to one endpoint, no migration.

### DEC-2: What are the sort keys and the default?
**Resolution:** `recent` (updated_at DESC — the current/default behavior),
`oldest` (updated_at ASC), `alpha` (title ASC NULLS LAST), `most_messages`
(message_count DESC, updated_at DESC tiebreak). Missing/unknown → `recent`.
**Basis:** codebase — the audit's cluster-2b spec lists exactly
"recent/oldest/alpha/most-messages"; `recent` preserves the existing
`ORDER BY c.updated_at DESC` in `conversations.rs:143`.

### DEC-3: Do we add an FTS / trigram index for content search?
**Resolution:** No new index this scope; rely on `ILIKE` over
`message_contents`. Documented as a future optimization if per-user volume grows.
**Basis:** codebase — per-user conversation/message volume is bounded (owner-
scoped queries), no migration is in scope, and adding `pg_trgm`/`tsvector` is a
separate perf workstream. Avoids migration collision risk (DEC noted in
PLAN_AUDIT "Migration collisions").

### DEC-4: What granularity is the in-conversation find highlight?
**Resolution:** Message-level. The find bar computes matching message ids
(client-side over `Stores.Chat.messages`), shows "X of Y", and Next/Prev/Enter
scroll the matched message into view and apply a transient highlight ring to that
message container. No substring-level highlight inside rendered markdown.
**Basis:** convention — chat messages render through streamdown/markdown
(`ChatMessage`→extensions); injecting `<mark>` into that pipeline is invasive and
fragile. Message-level jump is the tractable, testable "find + jump to match"
the scope asks for.

### DEC-5: How is the find bar opened/closed?
**Resolution:** A header toggle button (in the conversation header trailing area)
AND `Cmd/Ctrl-F` while the conversation view is focused (preventDefault to
override native find, since our find covers the same content). `Esc` and a close
button dismiss it and clear the query/highlight.
**Basis:** convention — matches the existing header-affordance pattern
(`ChatHistoryPage` search toggle) and the keyboard-extension idiom
(`extensions/keyboard/extension.tsx` already binds Ctrl+K/Ctrl+Enter/Esc).

### DEC-6: What is the collapse threshold, and when is a message NOT clamped?
**Resolution:** Clamp a text bubble taller than ~24rem (384px) to that height
with a bottom fade + "Show more"/"Show less" toggle, collapsed by default.
NEVER clamp the message that is currently streaming (the last assistant message
while `Stores.Chat.isStreaming`) so live tokens stay visible; it becomes
collapsible once streaming completes.
**Basis:** codebase — mirrors the existing bounded-height affordance on the MCP
tool card (`max-h-40 overflow-auto`, `extension.tsx:103`); the streaming
exclusion protects the live-output UX the scroll effects in `ConversationPage`
already prioritize.

### DEC-7: How is the composer draft keyed, and how are edit/streaming handled?
**Resolution:** Key = `conversationId`, or the literal `new` when no conversation
exists (new-chat page). Restore the draft into the textarea only on mount for
that key AND only when the textarea is empty AND `Stores.Chat.editingMessage` is
null (so edit-prefill and regenerate-prefill are never clobbered). Save on input
directly (localStorage writes are cheap and synchronous — no debounce needed;
immediate save is also deterministic for the e2e), suppressed while
`editingMessage` is set. On successful send,
clear the draft for the active key AND the `new` bucket (covers the
new-chat→created transition).
**Basis:** codebase — `startEditMessage`/`startRegenerateMessage` prefill via
`TextStore.setText` (`Chat.store.ts:957,1023`); the register-a-fn pattern
(`Text.store.ts`) is reused to add `clearDraft`, called from the text
extension's existing `onMessageSent` hook (`extensions/text/extension.tsx:152`).

### DEC-8: Where are drafts stored?
**Resolution:** `localStorage` under the `ziee:chat-draft:<key>` namespace,
client-only.
**Basis:** convention — scope says "persist across NAVIGATION", not across
devices; no server/sync surface is warranted. Matches other client-only UI
persistence (right-panel width via local persistence).

### DEC-9: What clipboard payloads does paste-image accept?
**Resolution:** Only image items/files from `clipboardData` (mime `image/*`),
routed through `Stores.File.__state.uploadFiles(...)` with the same 100MB cap and
`FilesUpload` permission gate as `FileUploadButton`. Plain-text paste is left to
the textarea (not intercepted / not `preventDefault`-ed).
**Basis:** codebase — `FileUploadArea`/`FileUploadButton` are the single upload
path (`uploadFiles`, `MAX_FILE_SIZE`, `usePermission(FilesUpload)`); paste reuses
them so behavior/limits/permissions are identical.

### DEC-10: What UI element is the sort control, and where does it live?
**Resolution:** The kit `Select` (`@/components/ui` `Select`), 4 options, placed
in the `/chats` header next to the search input on wide layout and in the body
(above the list) on narrow layout — reusing the page-width-aware placement
already in `ChatHistoryPage`.
**Basis:** codebase — kit `Select` is the standard single-choice control
(`components/ui/kit/select.tsx`, used by `ModelSelector`, `FileVersionBar`);
`ChatHistoryPage` already has the narrow/wide affordance-placement machinery.

### DEC-11: Does content-search execute client-side or server-side, and how is the sidebar kept clean?
**Resolution:** Server-side. `ChatHistory.setSearchQuery` and a new `setSort`
both call `loadConversations(1)`, which passes `search`/`sort` to
`ApiClient.Conversation.list`; the returned rows populate `conversations`
directly (the client-side `filteredConversations` title filter is removed).
`recentConversations` is refreshed ONLY on an unfiltered, `recent`-sort, page-1
load, so the sidebar's recent widget never shows a filtered/reordered subset.
**Basis:** codebase — content is not loaded client-side, so search must be
server-side; `loadConversations` already dedupes in-flight calls
(`ChatHistory.store.ts:39`) and is the shared entry for the sidebar widget +
new-chat page.

### DEC-12: How does the jump-to-latest button decide visibility?
**Resolution:** Reuse the existing `isAtBottomRef` IntersectionObserver on the
`messagesEndRef` sentinel; surface its boolean into React state and show the
button only when NOT at bottom. Click calls
`messagesEndRef.scrollIntoView({ behavior: 'smooth' })`.
**Basis:** codebase — `ConversationPage.tsx:49-60` already observes exactly this
sentinel; the button consumes the same signal, no new scroll math.
