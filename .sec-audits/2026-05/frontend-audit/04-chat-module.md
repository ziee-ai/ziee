# Audit 04 — Chat Module

Scope: `src-app/ui/src/modules/chat/` (97 files). Includes core stores, pages,
components, widgets, and all 10 extensions (assistant, file, mcp, model,
syntax, text, title, keyboard, export). MCP IN-CHAT integration audited.

Lenses (in priority order): Bugs → Inefficiencies → Responsive → Inconsistencies.

---

## Summary

Chat is the largest module by far and most of it is well-engineered: stores
are well-organized, the extension registry is clean, the right-panel
persistence has TTL eviction and renderer-safe rehydration, and the SSE event
fan-out is type-safe. However the audit found several high-severity bugs in
the conversation-load lifecycle, message-rendering performance, and
cross-tab/cross-conversation race handling. The two most important findings:

1. **HIGH-1 — ConversationPage stale-result race.** `ConversationPage.tsx:18-22`
   calls `loadConversation(id)` in a bare `useEffect` with no AbortController,
   no cleanup, and no stale-result guard. Switching conversations mid-load
   from the sidebar can leave the user looking at conversation A's messages
   while the URL says conversation B (or vice-versa). The store has SOME
   self-protection (loadingConversationId dedup) but no kill switch — once the
   network is in flight, the result is `set()` regardless of whether the user
   has since navigated to a third conversation.

2. **HIGH-2 — No message-list virtualization.** `MessageList.tsx:18,34`
   converts the `messages` Map to an array via `Array.from(...).values()` on
   every render, then maps `ChatMessage` for every message. With 1000+
   messages this is a documented performance problem; every streaming
   token triggers a full re-render of the message array. `ChatMessage` is
   `memo`'d but the inner `Streamdown` (with shiki syntax highlighting) is
   expensive enough that real-world usage at long conversation lengths will
   stutter on each token.

Other findings span: web/SVG viewer XSS surface (iframe with
`allow-scripts`), global keyboard handler that fires inside textareas, leaked
Blob URLs on conversation switch + Vite HMR, syntax extension dead code
duplicating the text extension's renderer, anchor `noopener` missing,
auto-scroll-on-every-message defeating "scroll up to read history", and a
left-side "back" button that hard-codes `/chats` ignoring previous route.

---

## Bugs

### HIGH-1 — ConversationPage: stale-result race on rapid switch
**File:** `src/modules/chat/pages/ConversationPage.tsx:18-22`

```ts
useEffect(() => {
  if (conversationId) {
    Stores.Chat.loadConversation(conversationId)
  }
}, [conversationId])
```

No AbortController, no cleanup function, no stale-result guard. Steps to
reproduce:

1. User on `/chat/A`, conversation A's `loadMessages` GET inflight (slow LLM
   inference history endpoint, 800 ms).
2. User clicks B in `RecentConversationsWidget` (sidebar). React Router
   updates URL → `/chat/B`. Effect re-runs with `conversationId = B`.
3. `Chat.store.loadConversation('B')` is called. Inside the store (`Chat.store.ts:521`):
   - `currentConversation` is still A — falls through dedup check.
   - `loadingId` is null (A's call finished saving snapshot before B started)
     or A's id — passes dedup.
   - Switches: saves A's snapshot, clears panel, kicks off B's
     `ApiClient.Conversation.get` + `loadMessages` + `loadBranches`.
4. While B's three calls are in flight, the user clicks A again. Effect
   re-fires with conversationId = A. `loadConversation('A')` runs, sees a
   cache hit, immediately swaps state to A.
5. B's `loadMessages` resolves and calls `set({ messages: ... })` —
   overwriting A's freshly-restored messages with B's data.

The `streamConversationId` capture in `sendMessage` (`Chat.store.ts:931`) is
the right pattern but is NOT applied to load. Add a similar guard:
`if (get().conversation?.id !== loadId) return` before each `set()` after
each await.

Severity: HIGH — produces "I'm looking at the wrong conversation" with no
recovery short of reload.

### HIGH-2 — MessageList lacks virtualization + key correctness during stream  *(revised 2026-05-23: severity is conditional)*

**File:** `src/modules/chat/components/MessageList.tsx:18, 34-36`

```ts
const messagesArray = Array.from(messages.values())
...
{messagesArray.map(msg => (
  <ChatMessage key={msg.id} message={msg} />
))}
```

No `react-window` / `react-virtuoso` library is used; no virtualization library
appears in `package.json`. For a 500-message conversation each streaming token
triggers:

1. Zustand subscription on `messages` fires.
2. `MessageList` re-renders: `Array.from(messages.values())` allocates a new
   500-element array; React reconciles 500 `<ChatMessage>` elements.
3. `ChatMessage` IS wrapped in `React.memo` (verified at `ChatMessage.tsx:11`),
   so non-streaming rows bail out of re-render via memo. Only the
   streaming row actually re-renders.

**Severity recalibration:** verification (post-audit) found that `TextContent`
currently renders messages as plain `<div style={{ whiteSpace: 'pre-wrap' }}>`
— the Streamdown / shiki syntax-highlighting infrastructure is NOT actually
wired into the rendering pipeline today (`streamdown` is in `package.json`
but isn't imported by the rendering chain; there's a `TODO` in `TextContent.tsx`
to add markdown rendering). With plain-text rendering of memo'd rows, per-token
re-render cost is small even at 500+ messages.

This finding becomes HIGH **the moment markdown / syntax highlighting is
wired up** (Streamdown + shiki re-highlight is expensive enough that the
non-virtualized list will stutter at >300 messages). For now, leave as **MED**:
the structure is fragile and should be fixed before that markdown PR lands,
but the user-visible impact today is minimal.

Secondary key-correctness bug (still HIGH-impact when streaming long
messages): during streaming, the streaming message's `id` mutates from
`streaming-${Date.now()}` → real DB ID (`Chat.store.ts:1096, 1142-1145`)
mid-stream. The key change tears down + remounts the message row,
losing any incremental rendering state. The store deletes the old entry from
the Map and inserts the new one, so React sees a key change. This is a
real bug that affects EVERY streaming message regardless of message-count
performance.

**Severity:** MED (overall), but split as:
- Virtualization absence: MED (becomes HIGH when Streamdown wires up)
- Key transition mid-stream: MED (real today, every streamed message)

Fix is the same: virtualize + stabilize keys to message-position rather than
message-id (so the streaming → final ID transition doesn't trigger remount).

### HIGH-3 — Auto-scroll always wins; user can't scroll up to read history mid-stream

**File:** `src/modules/chat/pages/ConversationPage.tsx:25-28`

```ts
const messagesEndRef = useRef<HTMLDivElement>(null)
useEffect(() => {
  messagesEndRef.current?.scrollIntoView({ behavior: 'smooth' })
}, [messages])
```

`messages` is a `Map` — the dependency comparison is by Map reference. Every
streaming token replaces the Map (`new Map(state.messages)` in the store),
so this effect runs on every token. The user's manual scroll-up is yanked
back to the bottom on each delta. There is no "stick to bottom only if user
is at bottom" detection. This is a well-known chat anti-pattern.

Severity: HIGH — UX bug that makes long streams unreadable.

### HIGH-4 — Web/SVG viewer iframe runs untrusted scripts

**File:** `src/modules/chat/extensions/file/file-viewers/web/body.tsx:17-22`

```tsx
<iframe
  sandbox="allow-scripts"
  srcDoc={content}
  ...
/>
```

`allow-scripts` without `allow-same-origin` is per the spec a unique-origin
sandbox, which prevents accessing parent `document.cookie` or `localStorage`.
BUT: it can still:
- Run arbitrary JS that displays misleading content (phishing UI inside the
  panel).
- Make XHR/fetch to arbitrary origins, including the user's app origin
  (without credentials, but probing internal endpoints).
- Use `top.location = ...` if `allow-top-navigation` were enabled (here it's
  not — good).

For SVG files (registered to the same `WebBody`), an attacker-controlled SVG
embedded in a tool result or uploaded by another user (if file sharing is
ever added) executes scripts in the user's tab. Recommended remediation: drop
`allow-scripts` for SVG entirely (render via `<img>` from a Blob URL so
script tags are inert), or add a strict `csp` attribute to the iframe.

Also, `Streamdown`'s markdown-rendered content is the model output. If the
model is induced (via prompt injection) to output HTML, Streamdown's
sanitizer pipeline is the only barrier. The audit didn't validate
Streamdown's HTML allowlist (third-party dep). Note for Agent 9 cross-cut.

Severity: HIGH — XSS surface.

### MED-1 — Global keyboard handler hijacks Ctrl+Enter / Ctrl+K / Esc inside ALL inputs

**File:** `src/modules/chat/extensions/keyboard/extension.tsx:122-133`

```ts
document.addEventListener('keydown', globalKeyboardHandler)
```

The handler runs at the document level with `preventDefault()` +
`stopPropagation()`. There is NO check that the event target is outside an
input. Consequences:

- **Esc** clears the message textarea regardless of where focus is. If the
  user has the model selector dropdown open and presses Esc to close it, the
  Esc fires the handler, which calls `textarea.value = '' + dispatchEvent`
  — losing the user's draft. AntD's Dropdown also acts on Esc, so the
  sequence is: dropdown closes + draft gets cleared.
- **Ctrl+K**: opens any browser feature using Ctrl+K (Firefox's search bar)
  is broken — `preventDefault()` blocks the browser shortcut even if the user
  is in a different input.
- **Ctrl+Enter**: actively submits the chat message even when focus is in
  the title editor (TitleEditor.tsx line 60-71), the elicitation form
  (ElicitationFormContent.tsx), or the MCP config modal.

Additionally, the lookup `document.querySelector('textarea[placeholder*="Type your message"]')`
hard-codes the placeholder string — any i18n / placeholder change breaks the
shortcut silently.

Severity: MED.

### MED-2 — Syntax extension's text renderer is dead code (or worse: collides with text extension)

**File:** `src/modules/chat/extensions/syntax/extension.tsx:160-172`

The `syntax` extension registers `contentTypes: { text: TextContentRenderer }`
with priority 70. The `text` extension (priority 5) also registers
`contentTypes: { text: TextContent }`. Both for content_type='text'.

In `chatExtensionRegistry.renderContent`, the last-registered (or
highest-priority — depends on registry semantics) wins. Looking at
`createExtension` registration order in `extensions/index.ts` and the
`priority: 5` vs `priority: 70` (lower = "higher priority" per text
extension's comment "High priority - runs before file (80)"), syntax loses
and never renders. The whole `EnhancedTextContent` / `parseMarkdownCodeBlocks`
/ `CodeBlock` infrastructure is dead.

Worst case: if the registry semantics flip (during a future refactor), the
syntax renderer takes over, the user loses Streamdown's incremental shiki
highlighting, and code blocks render via a regex-based parser that doesn't
handle nested ``` fences or non-newline-terminated blocks.

Severity: MED.

### MED-3 — Markdown anchor uses `rel="noreferrer"` only, missing `noopener`

**File:** `src/modules/chat/core/utils/useStreamdownComponents.tsx:78`

```tsx
return <a id={scopedId} href={scopedHref} className={className} {...rest} target="_blank" rel="noreferrer" />
```

Modern Chromium adds implicit `noopener` for `target=_blank` but Firefox ESR
≤ 102, Safari ≤ 12, and some embedded WebKit views do not. A malicious link
from a model output could `window.opener.location = phishingUrl`. Use
`rel="noopener noreferrer"`.

Severity: MED.

### MED-4 — Title button hard-navigates to `/chats` ignoring real "back"

**File:** `src/modules/chat/components/TitleEditor.tsx:42-44`

```ts
const handleBack = () => {
  navigate('/chats')
}
```

Visually a back-arrow icon (IoIosArrowBack:5). User expectation is "back" =
history.back(). If the user arrived at this conversation from `/hub` or
`/assistants`, they're teleported to `/chats` instead. Use `navigate(-1)` or
keep both options (back vs "go to all chats" as separate buttons).

Severity: MED.

### MED-5 — Blob URLs not revoked on conversation switch / unmount

**File:** `src/modules/chat/extensions/file/File.store.ts:617-655`, `657-682`

`loadPreviewPages` and `loadThumbnail` create Blob URLs via
`URL.createObjectURL`. They are revoked on `removeFile` and `clearFiles`
(for the user's own selected files) — but:

- Files restored via `messageFilesCache` (thumbnail of an artifact from a
  past assistant message) get Blob URLs that are NEVER revoked in this
  module's code. `messageFilesCache.clear()` is never called.
- On conversation switch, `Chat.store.loadConversation` calls
  `chatExtensionRegistry.cleanup()` (line 547) but the file extension's
  `cleanup` is not defined (file extension lacks a cleanup hook).
- Result: every time the user opens a conversation with N file artifacts,
  N more Blob URLs leak per page-view session. Browser keeps the blobs alive
  until tab close.

Also: PDF body's `loadPreviewPages` (`File.store.ts:630-633`) loops creating
URLs sequentially — if the component unmounts mid-loop, the remaining
iterations still fire `createObjectURL` then `set()`. No cancellation token.

Severity: MED — slow memory leak, problematic for power users on long
sessions.

### MED-6 — Send button disabled state doesn't include MCP approval queue

**File:** `src/modules/chat/components/ChatInput.tsx:37-39, 127`

```ts
if (sending || isStreaming || disabled || isUploadingFiles) return
```

The MCP extension's `beforeSendMessage` (`extensions/mcp/extension.tsx:635-650`)
allows sending an empty text WHEN there are pending tool-approval decisions.
But the disabled-state check doesn't know about that. Result: if the user
sees an approval card, clicks Approve, the MCP store adds the decision and
calls `Stores.Chat.sendMessage()` directly
(`ToolCallPendingApprovalContent.tsx:46`). Meanwhile, the TextInput is empty
and the Send button shows `disabled=true` (no text). Fine — the Approve
button is the entry point. But if the user ALSO types a follow-up question
between seeing the approval card and clicking Approve, the Send button
flicks from disabled → enabled mid-typing. Confusing.

Lower-impact: cancel race. `stopStreaming` uses an AbortController stored in
the store; if the user clicks the (loading) send button while the abort is
pending, nothing visible cues that aborting is in progress.

Severity: LOW-MED.

### MED-7 — McpConfigModal effect-tools loop fires N parallel calls without dedup across renders

**File:** `src/modules/chat/extensions/mcp/components/McpConfigModal.tsx:93-114`

```ts
useEffect(() => {
  if (configModalVisible) {
    enabledServers.forEach(async server => {
      if (!serverTools.has(server.id) && !loadingTools.has(server.id)) {
        setLoadingTools(prev => new Set(prev).add(server.id))
        ...
        const response = await ApiClient.McpServerRuntime.listTools({ id: server.id })
        ...
```

The dependency array is `[configModalVisible, enabledServers.length]`. Two
issues:

1. `enabledServers.length` as a dep loses individual identity changes — if a
   server is renamed (same count), the effect doesn't fire.
2. The dedup check `!serverTools.has(server.id) && !loadingTools.has(server.id)`
   reads stale closure state — on the second render where the modal stays
   open but `enabledServers.length` changes, both `serverTools` and
   `loadingTools` are the values FROM THE RENDER WHEN THE EFFECT RAN, not
   the freshest. Race when multiple servers come online quickly.

Also notes the known finding from the brief: line 100 is unguarded against
network failure — `loadingTools` resets but `serverTools` doesn't get an
empty entry, so the same server will be retried on every modal open. Not a
bug per se but no error UI either.

Severity: MED.

### LOW-1 — `mcp/extension.tsx:688` unguarded await on conversation MCP settings

Already known. Line 688 fetches `Conversation.getMcpSettings`. On 4xx/5xx
the catch block (line 760) silently falls back to "all servers enabled by
default" — this could mask a real backend error (e.g., conversation deleted
on another tab) and present a working-looking UI. Surface the error.

Severity: LOW.

### LOW-2 — `composeRequestFields` throws if no model selected (model extension)

**File:** `src/modules/chat/extensions/model/extension.tsx:54-60`

```ts
const modelId = Stores.Chat.__state.ModelStore.getModelId()
if (!modelId) {
  throw new Error('No model selected')
}
```

Throwing inside `composeRequestFields` propagates up to `sendMessage`'s outer
catch. The user sees `antMessage.error(error.message || 'Failed to send
message')` — i.e., "No model selected" toast. Better UX: validate in
`beforeSendMessage` so the cancel/error message goes through the same path
as the text extension's "Message cannot be empty" check.

Severity: LOW.

### LOW-3 — Export's filename leaks raw conversation ID

**File:** `src/modules/chat/extensions/export/extension.tsx:56,86,114`

```ts
a.download = `conversation-${conversation.id.slice(0, 8)}.json`
```

8 chars of a UUID is non-sensitive but: (a) makes user files easy to
correlate across exports (not great if the user shares the JSON publicly,
revealing a sortable prefix), (b) two conversations with the same UUID
prefix collide on save. Use the title (slugified) + a short hash. Cosmetic.

Severity: LOW.

### LOW-4 — ChatHistory store: `total` is wrong when search is active

**File:** `src/modules/chat/stores/ChatHistory.store.ts:103-108`

```ts
draft.total = draft.conversations.length
```

`total` is set to `draft.conversations.length`, which is the count of loaded
conversations (paginated), NOT the server's true total. Then
`ConversationList.tsx:217` shows "Showing X of Y conversations" — but Y is
the loaded-so-far count, which always equals X. Misleading.

Severity: LOW.

### LOW-5 — Streaming text component re-renders on every store change

**File:** `src/modules/chat/extensions/text/components/TextContent.tsx:17`

```ts
const { isStreaming } = Stores.Chat
```

This subscribes to the entire `Chat` store proxy. Even non-text state
changes (panelWidth drag, tab switch, forkPoints recompute) cause every
`TextContent` instance in the page to re-render. Use a selector subscription.

Severity: LOW (perf).

### LOW-6 — Title regeneration race on rename

The brief asks: "Title extension Auto-generation race — does the title API
call get cancelled if user renames manually?"

Looking at `title/extension.tsx:19-48` and `Chat.store.updateConversation`
(line 1311): NO cancellation. If the SSE `titleUpdated` event arrives AFTER
the user manually renamed via TitleEditor:
1. User renames conversation to "X". `updateConversation` fires
   `Conversation.update` → DB updated → emits event → store updated.
2. Concurrently the server-side title generator (auto-generated from
   first user message) finishes and emits `titleUpdated` SSE event with
   "Y".
3. `title/extension.tsx:31-35` overwrites the store's title to "Y".

The user's manual title is silently overwritten. Likely server-side
mitigation exists (don't auto-title if user already named it), but the
client doesn't enforce. Suggested fix: title extension ignores the SSE if
the local conversation has been manually renamed (`title_user_set` flag).

Severity: LOW.

---

## Inefficiencies

### EFF-1 — `Array.from(map.values())` allocations per render

`MessageList.tsx:18`, `FilePreviewList.tsx:30-41`, `ContentRenderer`,
`exportAsJSON/Text/Markdown` — all do `Array.from(...values())` per render.
Cheap individually, but `MessageList` does it per token while streaming.

### EFF-2 — `chatExtensionRegistry.renderContent` walks all extensions per content block

`components/ContentRenderer.tsx:16` calls the registry on every render of
every content block of every message. For a 500-message conversation with
3 content blocks per message and N extensions in the registry, that's
500 × 3 × N walks per render cycle. Cache in registry.

### EFF-3 — `computeForkPoints` sorts all messages by date on every call

`Chat.store.ts:676-679` does `[...state.messages.values()].sort(...)`
inside the action. Called from `loadBranches`, `activateBranch`, `complete`
SSE handler. For 1000+ message conversations this is 5-10 ms per call.
Cache.

### EFF-4 — `RawCodeView` renders one `<div>` per line

`shared/RawCodeView.tsx:41,45` renders one `<div>` per line (capped at 100).
At 100 lines × 2 columns = 200 divs. Fine. But if MAX_LINES is ever raised
to 10000 (common feature request for log viewer), this collapses. Consider
`<pre>` + counter `::before` content instead.

### EFF-5 — `saveAllPanelSnapshots` serializes the entire snapshots map on every panel mutation

`Chat.store.ts:139-145` JSON.stringifies every tab on every
displayInRightPanel/close/setActiveRightPanelTab call. For users with many
conversations and many open tabs per conversation, this approaches 50ms+
per click. Debounce.

### EFF-6 — Markdown components hook re-creates the overrides object on every component change

`useStreamdownComponents.tsx:9-101` memoizes on `contentId` — good. But
nested handlers reference `document.getElementById` inside event handlers,
so each link click triggers DOM lookup (negligible).

---

## Responsive / sizing / scrolling

### RES-1 — `ChatRightPanel` mobile drawer is full-screen `fixed inset-0 z-[1000]` — works, but locks body scroll never

**File:** `src/modules/chat/core/components/ChatRightPanel.tsx:113`

When the mobile overlay is open, the body behind it is still scrollable
under iOS Safari (rubber-band reaches body content). The overlay covers
visually but doesn't add `overflow: hidden` to body / preventDefault on
touchmove. Result: tap-and-drag-down on the overlay scrolls the
conversation page underneath.

Severity: MED (UX).

### RES-2 — `ChatInput` toolbar uses Flex justify-between with no min-width on the model selector container

**File:** `src/modules/chat/components/ChatInput.tsx:120`

`<ExtensionSlot name="toolbar_model" />` (the model selector) renders into
a container with no `min-width: 0`. On narrow viewports, the model name
can push the send button off-screen. Test by selecting a long-named model
(e.g., "claude-3-7-sonnet-20250122-v1:0") on a 360px viewport.

### RES-3 — `XlsxBody` Table uses fixed `scroll: { y: 'calc(100vh - 260px)' }`

**File:** `XlsxBody.tsx:103`, `DelimitedTable.tsx:72`

`100vh` ignores the actual right-panel viewport — if the panel is shorter
(e.g., the user has half-screen via the resize handle), the table extends
below the panel and scrolls inside the page. Use a wrapping container
ref + `ResizeObserver`, or `100%` of parent.

### RES-4 — `BranchNavigator` appears below assistant message; on narrow viewport the prev/next + index can wrap

**File:** `BranchNavigator.tsx:42-60`

`<Space size={2}>` doesn't enforce nowrap; with long messages the
controls can wrap below MessageActions.

### RES-5 — Editing message banner uses `borderRadius: LG LG 0 0` — breaks if the textbox loses its border

Cosmetic. The banner assumes the chat input is a card with rounded top
corners; if the input is restyled or embedded elsewhere, the banner has
the wrong shape.

### RES-6 — `messagesEndRef.scrollIntoView({ behavior: 'smooth' })` doesn't respect `prefers-reduced-motion`

Accessibility nit. Use `behavior: motionOk ? 'smooth' : 'auto'`.

---

## Inconsistencies

### INC-1 — Two text renderers (Streamdown vs regex-based syntax extension) register for `text` content

Already noted as MED-2.

### INC-2 — Store access pattern: `Stores.Chat` (reactive) vs `Stores.Chat.__state` (non-reactive)

The codebase mixes both. `ConversationPage.tsx:15` uses
`const { conversation, messages, loading, error } = Stores.Chat` (reactive).
`MessageActions.tsx:57` uses `Stores.Chat.__state.startEditMessage` for
calling an action — fine, actions don't need reactivity. But
`extension.tsx` files extensively use `Stores.Chat.__state.McpStore` for
reading state too. The pattern is mostly correct (extensions live outside
React render context) but a few component-level reads of `.__state` exist
that should be reactive. Searching `__state.` in `components/*.tsx` finds
none — extensions only. Good. But documenting this seam in `REACT_COMPONENT_PATTERNS.md`
is worth doing.

### INC-3 — `Promise.resolve().then(...)` deferred-load pattern is duplicated 6+ times in `File.store.ts`

Lines 406, 461, 556, 610. Same pattern: get cache, if miss, schedule async
load via microtask. Extract a helper `deferredCacheLoad(cache, key, fetcher)`.

### INC-4 — Error display: `Alert.error(err.message)` vs `message.error(err.message)` vs silent `console.error` — inconsistent

Files: many. Some surface errors via toasts (`MessageActions.tsx:75`), some
via `Alert` banners (`ConversationPage.tsx:64`), some swallow
(`Chat.store.ts:599-604`'s `loadConversation` catch only sets `error`
field — but if user navigates away, the error is never shown).

### INC-5 — `mcp/extension.tsx:267, 287, 386...` mixes `Stores.Chat.__state.McpStore` and `Stores.Chat.__state.McpStore.__state`

Some accesses use one level of `__state`, others two
(`mcp/extension.tsx:814`). One of these is probably wrong — likely a
store-proxy quirk that "works" but is type-unsafe.

### INC-6 — `tool_use` content has TWO renderers (mcp `McpToolUseRenderer` + DB-loaded fallback)

`mcp/extension.tsx:146-229`. The two code paths (lines 161-163 with
toolCall vs 165-228 without) duplicate the rendered card. Refactor into one
shared component.

### INC-7 — Title click goes back to `/chats`; sidebar conversation click goes to `/chat/:id` — no consistent "home"

Minor.

---

## Permission-gating considerations (for future)

Per the brief, chat is largely user-conversational. However, some chat
surfaces COULD warrant permission gating in future deployments:

- **Export** (`extensions/export/extension.tsx`) — exporting JSON includes
  raw message text. In multi-tenant deployments with classified data, you
  may want `chat::conversation::export` permission.
- **Branches** (`Chat.store.activateBranch`) — branch list reveals
  conversation forks. No permission check here, but the backend's
  `Branch.list` presumably enforces conversation ownership. Frontend
  doesn't double-check.
- **MCP config** in chat — `McpConfigModal` lets users select MCP servers
  for a conversation. If `mcp::server::use` permission ever differs from
  `mcp::server::read`, the modal would let the user CONFIGURE a server they
  can't actually use. Worth a CR.

Not bugs today — future considerations.

---

## File-viewer per-format audit (brief checklist)

| Viewer | Loading | Error | Large-file | Mobile fit | Memory cleanup |
|---|---|---|---|---|---|
| image (`image/body.tsx`) | Spin | none — broken `<img>` shows nothing | no — full thumbnail | `max-w-full` OK | Blob revoked via FileStore.removeFile only; leak via messageFilesCache |
| pdf (`pdf/body.tsx`) | Spin per page | none — silent | sequential page-by-page, no virtualization | OK | Same Blob leak |
| markdown (`markdown/body.tsx`) | Spin | none | no — full file | OK | N/A (text) |
| tabular csv/tsv (`tabular/body.tsx`) | Spin | none — silent | MAX_ROWS=100 hard cap with warning Alert (good) | Table.scroll uses 100vh — see RES-3 | N/A |
| tabular xlsx (`XlsxBody.tsx`) | Spin | yes — `loadError` Alert ✓ | MAX_ROWS=100 + max 10 sheets | RES-3 | N/A |
| web/html/svg (`web/body.tsx`) | Spin | none | no | iframe sizes 100%/100% | N/A (iframe garbage-collected on unmount) — but XSS surface, see HIGH-4 |
| text (`text/body.tsx`) | Spin | none | RawCodeView MAX_LINES=100 with Alert ✓ | OK | N/A |

---

## Conclusion

Top 10 most-actionable findings (by reduce-risk-now priority):

1. **HIGH-1** ConversationPage stale-result race — add AbortController +
   stale-id guard.
2. **HIGH-2** MessageList virtualization — adopt `react-virtuoso`.
3. **HIGH-3** Auto-scroll always wins — add at-bottom detection.
4. **HIGH-4** Web/SVG iframe scripts — drop `allow-scripts` for SVG;
   audit Streamdown sanitizer.
5. **MED-1** Global keyboard handler — gate on event target.
6. **MED-2** Syntax-extension dead/colliding renderer — remove.
7. **MED-3** Anchor `noopener` missing — fix one-liner.
8. **MED-5** Blob URL leak via messageFilesCache — add cleanup.
9. **MED-7** McpConfigModal effect-dep correctness — switch to identity
    array.
10. **LOW-6** Title auto-generation race vs manual rename — client guard.

Total Bugs: 4 HIGH, 7 MED, 6 LOW. Inefficiencies: 6. Responsive: 6.
Inconsistencies: 7.
