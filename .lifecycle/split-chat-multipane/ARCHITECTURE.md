# ARCHITECTURE — split-chat-multipane

Concrete code architecture for the per-pane refactor. Grounded in the existing
store-kit (`core/store-kit.ts`), the chat-extension registry
(`core/extensions/registry.tsx`), the stream client
(`core/stream/ChatStreamClient.ts`), and the backend chat-stream registry
(`server/.../chat/stream/registry.rs`). Companion to PLAN.md / DECISIONS.md.

## 1. Three ownership tiers

```
GLOBAL (singletons — live once)             TRANSPORT (process-global, shared)
  SplitView.store   layout: panes[],          streamAuthGuard  1 auth sub →
    focusedPaneId, dividerWidths, mode           start/stop ALL pane clients
  ChatHistory / Auth / AppLayout              EventBus
  ExtensionCatalog  descriptors only          paneRegistry: Map<paneId, Handle>
  Stores.Chat       BRIDGE → focused pane

PER-PANE (one set per open pane, keyed by paneId, built by <ChatPaneProvider>)
  ChatPaneStore  (defineLocalStore instance)   ChatStreamClient (factory instance)
  PaneExtensionRuntime  + per-pane extension store instances
```

**Invariant:** *descriptors are global, runtime is per-pane.* Which extensions
exist and their slots/handlers/renderers register once at module load into the
`ExtensionCatalog`; the store instances + `initialized` flag + lifecycle live in
a per-pane `PaneExtensionRuntime`.

## 2. Key contracts

```ts
// --- SplitView: the ONLY new global state -----------------------------------
interface Pane { paneId: string; conversationId: string | null; projectId: string | null }
interface SplitViewState {
  panes: Pane[]; focusedPaneId: string | null
  dividerWidths: number[]; mode: 'split' | 'tabs'
}
// actions: openPane, closePane, focusPane, setPaneConversation, reorderPanes, setDividerWidth, setMode

// --- The handle a pane subtree resolves -------------------------------------
interface ChatPaneHandle {
  paneId: string
  store: LocalStoreInstance<ChatPaneState>   // reactive reads + actions + .$
  runtime: PaneExtensionRuntime
  stream: ChatStreamClient
}
const useChatPane = (): ChatPaneHandle => useContext(ChatPaneContext)   // throws outside a pane

// --- Extension API migration (clean cut, DEC-30) ----------------------------
interface PaneExtensionCtx {
  paneId: string
  chatStore: StoreApi<ChatPaneState>   // subscribe/getState of THIS pane
  store<T>(): T                        // THIS pane's own extension store instance
  projectId: string | null            // replaces window.location derivation (ITEM-13)
  displayInRightPanel(tab: RightPanelTab): void
}
interface ChatExtension { initialize?(ctx: PaneExtensionCtx): Promise<void>; /* hooks take ctx */ }
// every extension: `import { useChatStore }` → `ctx.chatStore`; `Stores.Chat.MyStore` → `ctx.store()`
// the useChatStore singleton export is REMOVED.

// --- Stream: one connection per pane (factory over today's module singleton) -
function createChatStreamClient(o: {
  getConversationId(): string | null
  onFrame(conversationId: string, event: SSEEvent): void
  onReconnect(): void
}): { setActiveConversation(id: string | null): void; start(): void; stop(): void }
```

## 3. Assembling a pane — the wiring order that breaks the cycles

`<ChatPaneProvider paneId conversationId>` on mount:
1. `store = ChatPaneStore.use({ paneId, conversationId })` — own `local:<n>`
   EventBus group; **its own `frameApplyTail`** (moved out of module scope — this
   is what stops pane A's slow extension hook from stalling pane B's tokens).
2. `runtime = new PaneExtensionRuntime(ExtensionCatalog, store)` — instantiates
   each extension's store, runs `ext.initialize(ctx)`.
3. `store.attachRuntime(runtime)` — resolves the **store↔runtime cycle**:
   `sendMessage` / `loadConversation` call `get().runtime.composeRequestFields()`
   etc. instead of the singleton `chatExtensionRegistry`.
4. `stream = createChatStreamClient({ getConversationId, onFrame: (cid,e) =>
   store.enqueueStreamFrame(cid,e), onReconnect })` — `enqueueStreamFrame` chains
   through the per-instance `frameApplyTail` (`tail = tail.then(() =>
   applyStreamFrame(cid,e))`); it must NOT call `applyStreamFrame` directly, or
   fast frames interleave and drop/dup tokens (the bug the code documents).
5. `paneRegistry.set(paneId, handle)` — so the bridge + keyboard resolve the
   focused pane.
Unmount reverses: `paneRegistry.delete` → `stream.stop()` → `store.__destroy__()`
(auto-unsubs) → `runtime.cleanup()`.

## 4. How the store + streaming work with 2 conversations open

### Today (singleton) — why 2 don't work
ONE `Stores.Chat` (one `conversation`/`messages`/`isStreaming`), ONE SSE
connection subscribed via `setActiveConversation(id)` to ONE conversation.
Frames arrive tagged with `conversation_id`; `applyStreamFrame` writes into that
single store and **drops** any frame whose id ≠ the active conversation. Viewing
B swaps A into `conversationStateCache`. Only one is ever live.

### After (per-pane) — with A and B open
```
   SERVER (unchanged)          ONE app window
 ┌───────────┐  conn A   ┌──── Pane A ────┐   ┌──── Pane B ────┐
 │ chat-stream│◄─sub:A───┤ StreamClient A │   │ StreamClient B │
 │  registry  │──convA──►│  onFrame(A,e)  │   │  onFrame(B,e)  │
 │  gen A ────┼─frames──►│ PaneStore A    │   │ PaneStore B    │
 │  gen B ────┼─frames──►│  conv=A msgs   │   │  conv=B msgs   │
 │            │◄─sub:B───┤  isStreaming   │   │  isStreaming   │
 └───────────┘  conn B   │  frameTail(A)  │   │  frameTail(B)  │
                         └────────────────┘   └────────────────┘
```
- **Two store instances** — each `<ChatPaneProvider>` → `ChatPaneStore.use()` →
  a full independent copy; separate EventBus groups; they can't see each other.
- **Two SSE connections** — A's subscribed to conv A, B's to conv B. The backend
  registry already holds ≤12 connections/user each scoped to one conversation, so
  the server sends A's tokens down conn A and B's down conn B. **No server change.**
- **Frames land only in their pane** — A's connection only carries conv-A frames,
  so `A.stream.onFrame → A.store.enqueueStreamFrame` (which chains through the
  per-instance `frameApplyTail` into `applyStreamFrame`, never a direct call); the `cid ===
  conversation?.id` guard remains as a cheap safety net, not the router.

### Trace — send in A while B is mid-stream
1. B streaming: gen-B frames → conn B → `B.store.applyStreamFrame` → B grows,
   `B.isStreaming=true`. A untouched.
2. Send in A: `A.store.sendMessage()` → `A.runtime.composeRequestFields()` →
   `ApiClient.Message.send({conversation:A})` → the backend starts a **separate**
   generation (generations are keyed per conversation → A and B run concurrently).
3. Gen-A frames → conn A → `A.store.applyStreamFrame` → A grows,
   `A.isStreaming=true`.
4. Both panes stream at once, each into its own store / scroll / composer;
   per-instance `frameApplyTail` keeps A's assembly from blocking B's.

Same conversation in two panes is forbidden (DEC-9 → focus the existing pane), so
no frame is ever double-applied.

## 5. Data flow for the other flows

- **Open 2nd pane (drag → Split-right):** `paneDnd` drop → `SplitView.openPane` →
  `SplitChatView` renders a new `<ChatPaneProvider>` → §3 steps.
- **Tear-off:** `paneDnd` detects a drop outside the window → `openConversationWindow`
  (Tauri `WebviewWindow`) + `SplitView.closePane` → the conversation becomes a fresh
  SPA with its own singleton — **zero shared state** (why pop-out needs none of §3).
- **Global affordance (export / Ctrl+Enter):** resolve
  `paneRegistry.get(SplitView.focusedPaneId)` and act on that handle; keyboard DOM
  queries scope to the focused pane's root (fixes the global-`querySelector` bug).

## 6. The hardest calls, settled

| Risk | Resolution |
|---|---|
| store ↔ runtime circular dep | provider builds store → `runtime(store)` → `store.attachRuntime` (non-reactive ref) |
| `frameApplyTail` was module-global | moved INTO the pane store instance → per-pane serialization |
| bridge reactive read across changing focus | bridge does snapshot + action only; `useFocusedChatPane()` for the ~2 reactive reads |
| resolve focusedPaneId → live instance | module-level `paneRegistry` populated by providers (mirrors the backend registry) |
| ~13 extensions on `useChatStore` | one lever: `initialize()` → `initialize(ctx)`; clean cut, `useChatStore` export removed |
| unenveloped raw extension events (titleUpdated/mcp/approval) | attributed by WHICH connection they arrive on — one connection per conversation makes this exact (why per-connection beats a single-connection subscription-set) |

No new framework: `defineLocalStore` (instances), `defineExtensionStore`
(per-pane ext stores — already per-call), the backend's per-connection stream
scoping, and a small `paneRegistry`. The one genuinely new concept is the
extension **catalog/runtime split**; everything else is relocation.
