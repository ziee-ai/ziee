import { type ComponentType, memo, type ReactNode } from 'react'
import {
  defineLocalStore,
  defineStore,
  type StoreInitCtx,
  type StoreSet,
} from '@/core/store-kit'
import { useMessageViewStateStore } from '@/modules/chat/core/stores/MessageViewState.store'
import { ApiClient } from '@/api-client'
import type {
  Branch,
  Conversation,
  MessageContent,
  MessageWithContent,
} from '@/api-client/types'
import type { SSEEvent } from '@/modules/chat/core/extensions/types'
import {
  type ChatStreamClient,
  createChatStreamClient,
} from '@/modules/chat/core/stream/ChatStreamClient'
import {
  computeChildAnchor,
  computeParentAnchor,
} from '@/modules/chat/core/utils/branchAnchor.utils'
import {
  appendWindow,
  firstMessageId,
  lastMessageId,
  mergeTailWindow,
  prependWindow,
  toOrderedMap,
} from '@/modules/chat/core/stores/messageWindow'
import { chatExtensionRegistry } from '@/modules/chat/extensions'

/** Default page size for a message-history window (mirrors the backend default). */
const MESSAGE_PAGE_SIZE = 30

// ── Right panel types ──────────────────────────────────────────────────────

/**
 * Map of panel renderer type keys → the props each renderer expects.
 *
 * Extensions augment this interface via declaration merging so the rest of
 * the API can statically link a `type` to the `data` shape it accepts:
 *
 *   declare module '@/modules/chat/core/stores/Chat.store' {
 *     interface PanelRendererMap {
 *       file: { fileId: string }
 *     }
 *   }
 *
 * Each extension also calls `registerPanelRenderer(type, { component, icon })`
 * at runtime (typically in its `initialize()` hook) so the panel can resolve
 * the React component when rendering or rehydrating tabs.
 */
export interface PanelRendererMap {}

export type PanelType = keyof PanelRendererMap

export interface PanelRenderer<T extends PanelType> {
  component: ComponentType<PanelRendererMap[T]>
  icon?: ReactNode
}

/**
 * A right-panel tab record.
 *
 * Fully serializable — `data` carries everything the renderer needs to
 * reconstruct the view. There is intentionally no `component` field; the
 * panel resolves it through `panelRendererRegistry` keyed by `type`.
 */
export interface RightPanelTab<T extends PanelType = PanelType> {
  id: string
  title: string
  type: T
  data: PanelRendererMap[T]
}

interface ConversationPanelSnapshot {
  tabs: RightPanelTab[]
  activeId: string | null
  /**
   * ms epoch of last user access. Updated when the user navigates to the
   * conversation (touchPanelSnapshot) or modifies its panel
   * (savePanelSnapshotForConversation). Snapshots older than PANEL_TTL_MS
   * are evicted on the next write.
   */
  lastAccessedAt: number
}

/**
 * Internal, type-erased view of panel data. The precise per-type shape
 * (`PanelRendererMap[T]`) lives only on the PUBLIC edges —
 * `registerPanelRenderer` / `displayInRightPanel` / `RightPanelTab<T>` —
 * where the caller supplies a concrete `T`. The registry storage and the
 * render boundary deliberately erase to this: indexing the map by
 * `PanelType` *here* would collapse to `never` whenever zero extensions are
 * loaded (e.g. the chat module type-checked in isolation), even though every
 * value flowing through is sound by construction.
 */
export type ErasedPanelData = Record<string, unknown>

interface ErasedPanelRenderer {
  component: ComponentType<ErasedPanelData>
  icon?: ReactNode
}

// Module-level registry of panel renderers, populated by extensions.
const panelRendererRegistry = new Map<string, ErasedPanelRenderer>()

export function registerPanelRenderer<T extends PanelType>(
  type: T,
  renderer: PanelRenderer<T>,
): void {
  // Auto-memoize the registered component so it only re-renders when its
  // serialized `data` props actually change. `ActivePanelContent` re-runs
  // on every rightPanel state change (width drag, drawer toggle, tab
  // activation, etc.), and most renderers (PDF, XLSX, image) are expensive
  // to re-mount; memoization gives every extension props-shallow-equality
  // render skipping by default.
  panelRendererRegistry.set(type, {
    ...renderer,
    // memo(...) returns a MemoExoticComponent which is structurally a
    // ComponentType; widen the precise PanelRendererMap[T] props to the
    // erased storage shape. Sound: the public <T> signature already proved
    // `component` accepts PanelRendererMap[T], a subtype of ErasedPanelData.
    component: memo(
      renderer.component,
    ) as unknown as ComponentType<ErasedPanelData>,
  })
}

/**
 * Resolve a tab's renderer to its component + icon. Returns null and warns
 * (in dev) if no renderer is registered for the tab's type — this typically
 * means the owning extension hasn't initialized yet, or the type was removed.
 */
export function resolvePanelRenderer(tab: RightPanelTab): {
  Component: ComponentType<ErasedPanelData>
  icon?: ReactNode
} | null {
  const renderer = panelRendererRegistry.get(tab.type)
  if (!renderer) {
    if (import.meta.env.DEV) {
      console.warn(
        `[ChatRightPanel] No renderer registered for type "${String(tab.type)}" — ` +
          `tab "${tab.title}" will not render. Make sure the owning extension ` +
          `calls registerPanelRenderer in its initialize() hook.`,
      )
    }
    return null
  }
  return { Component: renderer.component, icon: renderer.icon }
}

// ── localStorage helpers ───────────────────────────────────────────────────

// v2 bump: tab shape changed from { id, title } to { id, title, type, data }.
// Old v1 snapshots are discarded (panel state, not user data).
const PANEL_STORAGE_KEY = 'ziee-right-panel-tabs-v2'

/**
 * Snapshots not touched within this window are evicted on the next write.
 * Bounds storage growth from deleted/stale conversations whose snapshots
 * would otherwise live forever as orphans.
 */
const PANEL_TTL_MS = 30 * 24 * 60 * 60 * 1000 // 30 days

// ── Chat-token stream module state ──────────────────────────────────────────
// Min interval between reconnect-driven open-conversation refetches, so a
// flapping stream can't storm `loadMessages` (mirrors SyncClient's debounce).
// `frameApplyTail` (frame-apply serialization), `lastChatResyncAt` (resync
// throttle) and the stream client were formerly MODULE-scope singletons; they
// are now PER-INSTANCE (declared in `init`) so each split pane serializes /
// throttles / streams independently and one pane's teardown can't disturb
// another's (ITEM-6).
const CHAT_RESYNC_MIN_INTERVAL_MS = 5_000

function loadAllPanelSnapshots(): Record<string, ConversationPanelSnapshot> {
  try {
    const raw = localStorage.getItem(PANEL_STORAGE_KEY)
    if (!raw) return {}
    return JSON.parse(raw) as Record<string, ConversationPanelSnapshot>
  } catch {
    return {}
  }
}

function saveAllPanelSnapshots(
  snapshots: Record<string, ConversationPanelSnapshot>,
): void {
  try {
    localStorage.setItem(PANEL_STORAGE_KEY, JSON.stringify(snapshots))
  } catch {
    // Storage quota exceeded or unavailable — silently ignore
  }
}

/**
 * In-place evict snapshots older than PANEL_TTL_MS. Called at every write
 * point so the storage map self-cleans without a dedicated background task.
 * Entries that pre-date the lastAccessedAt field are treated as fresh
 * (timestamp = now) on first read in loadAllPanelSnapshots' caller path.
 */
function evictStaleSnapshots(
  snapshots: Record<string, ConversationPanelSnapshot>,
): void {
  const now = Date.now()
  const cutoff = now - PANEL_TTL_MS
  for (const [id, snap] of Object.entries(snapshots)) {
    // Pre-TTL entries (no lastAccessedAt) get a 30-day grace period rather
    // than being evicted immediately — backfill the timestamp to now so they
    // survive at least one full TTL window after this code first runs.
    if (snap.lastAccessedAt === undefined) {
      snap.lastAccessedAt = now
      continue
    }
    if (snap.lastAccessedAt < cutoff) {
      delete snapshots[id]
    }
  }
}

/**
 * Bump lastAccessedAt for a conversation without changing its tabs/activeId.
 * Called when the user navigates to a conversation that already has a
 * snapshot — without this, a conversation whose panel state is never modified
 * (user opens it daily but never touches the panel) would eventually be
 * evicted despite being actively used.
 */
function touchPanelSnapshot(conversationId: string): void {
  const all = loadAllPanelSnapshots()
  const snap = all[conversationId]
  if (!snap) return
  snap.lastAccessedAt = Date.now()
  evictStaleSnapshots(all)
  saveAllPanelSnapshots(all)
}

function savePanelSnapshotForConversation(
  conversationId: string,
  tabs: RightPanelTab[],
  activeId: string | null,
): void {
  const all = loadAllPanelSnapshots()
  if (tabs.length === 0) {
    delete all[conversationId]
  } else {
    const persistedIds = new Set(tabs.map(t => t.id))
    const persistedActiveId =
      activeId && persistedIds.has(activeId) ? activeId : (tabs[0]?.id ?? null)
    // Tabs are already serializable (no React values), persist as-is.
    all[conversationId] = {
      tabs,
      activeId: persistedActiveId,
      lastAccessedAt: Date.now(),
    }
  }
  // Opportunistic GC: every write is a chance to evict stale entries.
  evictStaleSnapshots(all)
  saveAllPanelSnapshots(all)
}

/**
 * Filter persisted tabs to those whose renderer is currently registered.
 * Tabs for unregistered types are silently dropped — typically that means
 * the extension that owned them hasn't loaded yet (e.g. lazy-loaded module
 * not pulled in for this route), so the tab simply won't appear.
 */
function rehydrateTabs(persisted: RightPanelTab[]): RightPanelTab[] {
  return persisted.filter(t => panelRendererRegistry.has(t.type))
}

/**
 * Snapshot of conversation state for caching
 */
interface ChatStateSnapshot {
  conversation: Conversation | null
  messages: Map<string, MessageWithContent>
  streamingMessage: MessageWithContent | null
  tempUserMessageId: string | null
  isStreaming: boolean
  // Preserve the lazy-load window boundaries so a cached conversation restores
  // with correct pagination affordances (without these, a restored conversation
  // couldn't scroll up to load older messages).
  hasMoreBefore: boolean
  hasMoreAfter: boolean
}

interface ChatState {
  // Data
  conversation: Conversation | null
  messages: Map<string, MessageWithContent>

  // Loading states
  loading: boolean
  loadingConversationId: string | null
  sending: boolean
  isStreaming: boolean
  error: string | null

  // ── Lazy-load window state ──────────────────────────────────────────────
  // The `messages` Map holds a contiguous slice of the active branch path.
  // These flags drive reverse-infinite-scroll (load older on scroll-up) and
  // the after= direction (load newer after an around= jump).
  /** Older messages exist before the oldest loaded one (show top spinner / paginate up). */
  hasMoreBefore: boolean
  /** Newer messages exist after the newest loaded one (only true after an around= jump). */
  hasMoreAfter: boolean
  /** An older-page fetch is in flight (guards the scroll trigger + shows the top spinner). */
  loadingOlder: boolean
  /** A newer-page fetch is in flight (re-entrancy guard for the bottom sentinel). */
  loadingNewer: boolean

  // Streaming message assembly
  streamingMessage: MessageWithContent | null
  tempUserMessageId: string | null

  // Conversation state cache (whole-store snapshots)
  conversationStateCache: Map<string, ChatStateSnapshot>
  cacheClearTimers: Map<string, NodeJS.Timeout>

  // ── Branch state ──────────────────────────────────────────────────────────

  /** All branches for the current conversation */
  branches: Branch[]
  branchesLoading: boolean

  /**
   * Message ID to create a new branch from on the next sendMessage call.
   * Set by startEditMessage (edit flow) and startRegenerateMessage (regenerate flow).
   * Cleared by clearPendingBranch() after the message is sent.
   */
  pendingBranchFromMessageId: string | null

  /**
   * The fork level for the next branch to be created.
   * - 'user': edit flow — navigator anchors at the edited user message bubble.
   * - 'assistant': regenerate flow — navigator anchors at the assistant bubble.
   * - null: no pending branch.
   */
  pendingBranchForkLevel: 'user' | 'assistant' | null

  /**
   * Per-branch fork level map.
   * Maps branchId → 'user' | 'assistant'.
   * Persists the fork level captured at branch creation so computeForkPoints
   * can determine the correct anchor even after pendingBranchForkLevel is cleared.
   * In-memory only — defaults to 'user' on page reload.
   */
  branchForkLevels: Map<string, 'user' | 'assistant'>

  /**
   * Set to true when the SSE 'started' event reveals a new branch was created.
   * Cleared in the complete SSE handler after reloading messages.
   */
  branchChangedDuringStream: boolean

  /**
   * Per-message fork points.
   * Maps messageId → ordered list of branch IDs that diverge at that message.
   * Used by BranchNavigator to render < X/N > at the right bubble.
   */
  forkPoints: Map<string, string[]>

  /**
   * The message currently being edited. Non-null puts the Chat Input into
   * edit mode — extensions subscribe to this field via Zustand subscribe
   * in their initialize() hooks to restore their state (e.g. files).
   */
  editingMessage: MessageWithContent | null

  // ── Conversation state management ────────────────────────────────────────

  saveConversationState: (conversationId: string) => void
  loadConversationState: (conversationId: string) => boolean
  scheduleCacheClear: (conversationId: string, delayMs?: number) => void
  cancelCacheClear: (conversationId: string) => void
  clearConversationCache: (conversationId: string) => void

  // ── Core actions ──────────────────────────────────────────────────────────

  createConversation: (
    title?: string,
    modelId?: string,
    /// Defer the `conversation.created` event. `sendMessage` uses
    /// this so extensions running on `afterCreateConversation` get
    /// to mutate the conversation BEFORE subscribers see the event.
    /// Default true (callers from buttons / drawers emit immediately).
    emitCreated?: boolean,
  ) => Promise<Conversation>
  loadConversation: (id: string) => Promise<void>
  /** Full (re)load of the newest page (tail) — resets the window. Used on
   *  initial open, branch switch, edit-cancel, and abort-reload. */
  loadMessages: (id: string) => Promise<void>
  /** Prepend the next OLDER page (before=oldest-loaded). Guarded by hasMoreBefore. */
  loadOlderMessages: () => Promise<void>
  /** Append the next NEWER page (after=newest-loaded). Guarded by hasMoreAfter
   *  (only relevant after an around= jump left us mid-conversation). */
  loadNewerMessages: () => Promise<void>
  /** Jump to a (possibly-unloaded) message: load a window CENTERED on it
   *  (around=) and replace the window. Returns false if the id isn't on the
   *  active branch. The caller scroll-centers + highlights it. */
  jumpToMessage: (messageId: string) => Promise<boolean>
  /** Merge the newest page into the window without discarding loaded older
   *  pages (used after a streamed turn / cross-device change). */
  reconcileTail: (conversationId: string) => Promise<void>
  sendMessage: () => Promise<void>
  applyStreamFrame: (conversationId: string, event: any) => Promise<void>
  updateConversation: (updates: { title?: string }) => Promise<void>
  clearError: () => void
  reset: () => void

  // ── Branch actions ────────────────────────────────────────────────────────

  loadBranches: (conversationId: string) => Promise<void>
  activateBranch: (conversationId: string, branchId: string) => Promise<void>
  computeForkPoints: () => Promise<void>
  trimMessagesToForkPoint: (forkMessageId: string) => void
  captureBranchForkLevel: (branchId: string) => void
  clearPendingBranch: () => void

  /**
   * Enter edit mode for a user message.
   * Trims messages to the fork point, pre-fills the text input, and emits
   * the editingMessage field change so extensions can restore their state.
   */
  startEditMessage: (messageId: string) => Promise<void>

  /**
   * Cancel edit mode without sending.
   * Clears editingMessage (extensions react via subscribe), clears the text
   * input, and reloads messages to restore what was trimmed.
   */
  cancelEdit: () => Promise<void>

  /**
   * Regenerate an assistant response on a new branch.
   * Finds the preceding user message, pre-fills text, trims, and auto-sends.
   */
  startRegenerateMessage: (assistantMessageId: string) => Promise<void>

  // ── Stop streaming ────────────────────────────────────────────────────────

  streamingAbortController: AbortController | null
  // The assistant message id of the in-flight generation (from the send
  // response), used to address the stop-generation endpoint.
  streamingMessageId: string | null
  /** This instance's own chat-token stream client (ITEM-6); null before init. */
  chatStreamClient: ChatStreamClient | null
  stopStreaming: () => void

  // ── Right panel ───────────────────────────────────────────────────────────

  rightPanel: {
    panelWidth: number
    tabs: RightPanelTab[]
    activeId: string | null
    mobileDrawerOpen: boolean
  }
  displayInRightPanel: <T extends PanelType>(entry: RightPanelTab<T>) => void
  /** Patch an existing right-panel tab's `data` in place (no-op if the tab is
   *  gone) and re-persist the conversation snapshot. `displayInRightPanel` only
   *  upserts/focuses; this is how an open panel (e.g. literature screening)
   *  saves evolving state so it survives reload. */
  updateRightPanelTab: <T extends PanelType>(id: string, data: PanelRendererMap[T]) => void
  setActiveRightPanelTab: (id: string) => void
  closeRightPanelTab: (id: string) => void
  closeAllRightPanelTabs: () => void
  closeMobileDrawer: () => void
  setRightPanelWidth: (width: number) => void

  // ── Lifecycle methods ─────────────────────────────────────────────────────

  __init__: {
    __store__?: () => void
  }
  __destroy__?: () => void
}

// Shared authoring config for the chat store. The SAME config builds BOTH the
// eager "primary" pane (a `defineStore` singleton — pane 0, keeps single-pane
// behaviour byte-identical + gives boot-time consumers/registry a store to bind
// to) AND per-pane `defineLocalStore` instances for additional split panes.
// The initial per-conversation state (named so `typeof chatInitialState` can
// type the actions/init callbacks — extracting the config to a const otherwise
// drops the contextual param typing `defineStore` gave them inline).
const chatInitialState = {
    conversation: null as Conversation | null,
    messages: new Map<string, MessageWithContent>(),
    loading: false,
    loadingConversationId: null as string | null,
    sending: false,
    isStreaming: false,
    error: null as string | null,
    hasMoreBefore: false,
    hasMoreAfter: false,
    loadingOlder: false,
    loadingNewer: false,
    streamingMessage: null as MessageWithContent | null,
    tempUserMessageId: null as string | null,
    streamingAbortController: null as AbortController | null,
    streamingMessageId: null as string | null,
    // This instance's own chat-token stream client (ITEM-6). Created in `init`
    // so actions can scope it via `setActiveConversation`; null before init.
    chatStreamClient: null as ChatStreamClient | null,
    conversationStateCache: new Map<string, ChatStateSnapshot>(),
    cacheClearTimers: new Map<string, NodeJS.Timeout>(),
    // Branch initial state
    branches: [] as Branch[],
    branchesLoading: false,
    pendingBranchFromMessageId: null as string | null,
    pendingBranchForkLevel: null as 'user' | 'assistant' | null,
    branchForkLevels: new Map<string, 'user' | 'assistant'>(),
    branchChangedDuringStream: false,
    forkPoints: new Map<string, string[]>(),
    editingMessage: null as MessageWithContent | null,
    // Right panel initial state
    rightPanel: {
      panelWidth: 440,
      tabs: [] as RightPanelTab[],
      activeId: null as string | null,
      mobileDrawerOpen: false,
    },
}

const chatStoreConfig = {
  state: chatInitialState,
  actions: (
    set: StoreSet<typeof chatInitialState>,
    getRaw: () => typeof chatInitialState,
  ) => {
    const get = getRaw as () => ChatState
    return {

    // ── Conversation state management ──────────────────────────────────────

    saveConversationState: (conversationId: string) => {
      const state = get()
      const snapshot: ChatStateSnapshot = {
        conversation: state.conversation,
        messages: new Map(state.messages),
        streamingMessage: state.streamingMessage,
        tempUserMessageId: state.tempUserMessageId,
        isStreaming: state.isStreaming,
        hasMoreBefore: state.hasMoreBefore,
        hasMoreAfter: state.hasMoreAfter,
      }
      set(state => {
        const newCache = new Map(state.conversationStateCache)
        newCache.set(conversationId, snapshot)
        return { conversationStateCache: newCache }
      })
      console.log(
        `[Chat.store] Saved conversation state for: ${conversationId}`,
      )
    },

    loadConversationState: (conversationId: string): boolean => {
      const state = get()
      const snapshot = state.conversationStateCache.get(conversationId)
      if (!snapshot) {
        console.log(
          `[Chat.store] Cache miss for conversation: ${conversationId}`,
        )
        return false
      }

      set({
        conversation: snapshot.conversation,
        messages: new Map(snapshot.messages),
        streamingMessage: snapshot.streamingMessage,
        tempUserMessageId: snapshot.tempUserMessageId,
        isStreaming: snapshot.isStreaming,
        hasMoreBefore: snapshot.hasMoreBefore ?? false,
        hasMoreAfter: snapshot.hasMoreAfter ?? false,
        loadingOlder: false,
        loadingNewer: false,
      })
      console.log(
        `[Chat.store] Cache hit - restored conversation state for: ${conversationId}`,
      )
      return true
    },

    scheduleCacheClear: (
      conversationId: string,
      delayMs: number = 5 * 60 * 1000,
    ) => {
      get().cancelCacheClear(conversationId)

      const timer = setTimeout(() => {
        get().clearConversationCache(conversationId)
        console.log(
          `[Chat.store] Auto-cleared cache for conversation: ${conversationId}`,
        )
      }, delayMs)

      set(state => {
        const newTimers = new Map(state.cacheClearTimers)
        newTimers.set(conversationId, timer)
        return { cacheClearTimers: newTimers }
      })
      const delayMinutes = Math.round(delayMs / 60000)
      console.log(
        `[Chat.store] Scheduled cache clear for ${conversationId} in ${delayMinutes} minute(s)`,
      )
    },

    cancelCacheClear: (conversationId: string) => {
      const state = get()
      const timer = state.cacheClearTimers.get(conversationId)
      if (timer) {
        clearTimeout(timer)
        set(state => {
          const newTimers = new Map(state.cacheClearTimers)
          newTimers.delete(conversationId)
          return { cacheClearTimers: newTimers }
        })
        console.log(
          `[Chat.store] Cancelled cache clear for conversation: ${conversationId}`,
        )
      }
    },

    clearConversationCache: (conversationId: string) => {
      get().cancelCacheClear(conversationId)
      set(state => {
        const newCache = new Map(state.conversationStateCache)
        newCache.delete(conversationId)
        return { conversationStateCache: newCache }
      })
      console.log(
        `[Chat.store] Cleared cache for conversation: ${conversationId}`,
      )
    },

    // ── Core actions ───────────────────────────────────────────────────────

    createConversation: async (
      title?: string,
      modelId?: string,
      emitCreated: boolean = true,
    ) => {
      // Extensions can layer additional attribution onto the
      // freshly-created conversation via the
      // `afterCreateConversation` hook in sendMessage.
      set({ loading: true, error: null })

      try {
        const conversation = await ApiClient.Conversation.create({
          title: title,
          model_id: modelId,
        })
        set({ conversation, loading: false })

        if (emitCreated) {
          const { Stores } = await import('@/core/stores')
          await Stores.EventBus.emit({
            type: 'conversation.created',
            data: { conversation },
          })
        }

        return conversation
      } catch (error: any) {
        set({
          error: error.message || 'Failed to create conversation',
          loading: false,
        })
        throw error
      }
    },

    loadConversation: async (id: string) => {
      // Scope this device's token stream to the conversation being opened, so
      // it receives (only) this conversation's live assistant tokens — and a
      // catch-up replay if it is mid-generation. Deduped for a no-op repeat.
      void get().chatStreamClient?.setActiveConversation(id)

      const currentConversation = get().conversation
      const loadingId = get().loadingConversationId

      if (currentConversation && currentConversation.id === id) {
        console.log(`[Chat.store] Conversation ${id} already loaded, skipping`)
        return
      }

      if (loadingId === id) {
        console.log(
          `[Chat.store] Conversation ${id} is already loading, skipping`,
        )
        return
      }

      if (currentConversation && currentConversation.id !== id) {
        console.log(
          `[Chat.store] Switching from ${currentConversation.id} to ${id} - saving current state`,
        )
        get().saveConversationState(currentConversation.id)
        get().scheduleCacheClear(currentConversation.id)

        // Save outgoing conversation's panel tabs to localStorage, then clear panel
        const { rightPanel } = get()
        savePanelSnapshotForConversation(
          currentConversation.id,
          rightPanel.tabs,
          rightPanel.activeId,
        )
        set(state => ({
          rightPanel: {
            ...state.rightPanel,
            tabs: [],
            activeId: null,
            mobileDrawerOpen: false,
          },
        }))

        await chatExtensionRegistry.cleanup()
        // Clear messages on switch so consumers never momentarily see the
        // OUTGOING conversation's messages under the new conversation id.
        // (Outgoing state was already saved via saveConversationState above;
        // the cache-hit/miss paths below repopulate from cache or the API.)
        // Without this, ConversationPage's first-load scroll latches against
        // the stale Map and the new conversation gets an animated
        // scroll-through that defeats inline-file lazy-loading.
        set({
          isStreaming: false,
          sending: false,
          streamingMessage: null,
          tempUserMessageId: null,
          streamingAbortController: null,
          streamingMessageId: null,
          messages: new Map(),
          hasMoreBefore: false,
          hasMoreAfter: false,
          loadingOlder: false,
          loadingNewer: false,
        })
        // Drop the outgoing conversation's ephemeral per-row view state
        // (show-more collapse, inline-file collapse/seen/height) so the
        // incoming conversation starts clean — message ids are globally unique,
        // so this is a memory-bound + correctness measure, not required for
        // isolation (message-scroll-stability, ITEM-6 / DEC-4).
        useMessageViewStateStore.getState().resetViewState()
      }

      get().cancelCacheClear(id)

      const cacheHit = get().loadConversationState(id)
      if (cacheHit) {
        console.log(`[Chat.store] Cache hit for conversation: ${id}`)
        await chatExtensionRegistry.initialize()

        const { conversation } = get()
        if (conversation) {
          await chatExtensionRegistry.onConversationLoad(conversation)
          await get().loadBranches(id)
        }

        // Restore panel tabs from localStorage (after initialize() so registry is populated)
        const panelSnapshot = loadAllPanelSnapshots()[id]
        if (panelSnapshot) {
          const tabs = rehydrateTabs(panelSnapshot.tabs)
          if (tabs.length > 0) {
            set(state => ({
              rightPanel: {
                ...state.rightPanel,
                tabs,
                activeId: panelSnapshot.activeId,
              },
            }))
          }
          // Bump lastAccessedAt so the snapshot isn't evicted just because
          // the user keeps revisiting without modifying the panel.
          touchPanelSnapshot(id)
        }
        return
      }

      console.log(`[Chat.store] Cache miss for conversation: ${id}`)
      set({ loading: true, loadingConversationId: id, error: null })
      try {
        const conversation = await ApiClient.Conversation.get({ id })
        // Stale-result guard: if the user navigated away during the
        // await (loadingConversationId changed), drop this response.
        // Prevents the A→B→A race where A's slow response overwrites
        // B's freshly-loaded conversation. (audit 04 HIGH-1 mitigation)
        if (get().loadingConversationId !== id) {
          console.log(`[Chat.store] Stale response for ${id}, dropping`)
          return
        }
        set({ conversation, loading: false, loadingConversationId: null })

        await get().loadMessages(id)
        if (get().conversation?.id !== id) return
        await get().loadBranches(id)
        if (get().conversation?.id !== id) return

        await chatExtensionRegistry.initialize()
        await chatExtensionRegistry.onConversationLoad(conversation)

        // Restore panel tabs from localStorage (after initialize() so registry is populated)
        const panelSnapshot = loadAllPanelSnapshots()[id]
        if (panelSnapshot) {
          const tabs = rehydrateTabs(panelSnapshot.tabs)
          if (tabs.length > 0) {
            set(state => ({
              rightPanel: {
                ...state.rightPanel,
                tabs,
                activeId: panelSnapshot.activeId,
              },
            }))
          }
          touchPanelSnapshot(id)
        }
      } catch (error: any) {
        // Only surface error if we're still on this conversation; an
        // abort from navigation is not a user-facing error.
        if (get().loadingConversationId === id) {
          set({
            error: error.message || 'Failed to load conversation',
            loading: false,
            loadingConversationId: null,
          })
        }
      }
    },

    loadMessages: async (id: string) => {
      set({ loading: true, error: null })
      try {
        // Newest page (tail): no cursor. Resets the window.
        const page = await ApiClient.Message.getHistory({
          id,
          limit: MESSAGE_PAGE_SIZE,
        })
        set({
          messages: toOrderedMap(page.messages),
          hasMoreBefore: page.has_more_before,
          hasMoreAfter: page.has_more_after,
          loadingOlder: false,
          loadingNewer: false,
          loading: false,
        })
      } catch (error: any) {
        set({
          error: error.message || 'Failed to load messages',
          loading: false,
        })
      }
    },

    loadOlderMessages: async () => {
      const state = get()
      const conversationId = state.conversation?.id
      // Guard: nothing older, already fetching, mid-stream (the live buffer is
      // authoritative), or empty window.
      if (
        !conversationId ||
        !state.hasMoreBefore ||
        state.loadingOlder ||
        state.isStreaming
      ) {
        return
      }
      const oldestId = firstMessageId(state.messages)
      if (!oldestId) return

      set({ loadingOlder: true })
      try {
        const page = await ApiClient.Message.getHistory({
          id: conversationId,
          before: oldestId,
          limit: MESSAGE_PAGE_SIZE,
        })
        // Drop the result if the user switched conversations mid-fetch.
        if (get().conversation?.id !== conversationId) return
        set(s => ({
          messages: prependWindow(s.messages, page.messages),
          hasMoreBefore: page.has_more_before,
          loadingOlder: false,
        }))
        await get().computeForkPoints()
      } catch (error: any) {
        if (get().conversation?.id === conversationId) {
          set({
            error: error.message || 'Failed to load older messages',
            loadingOlder: false,
          })
        }
      }
    },

    loadNewerMessages: async () => {
      const state = get()
      const conversationId = state.conversation?.id
      // Re-entrancy guard (`loadingNewer`) mirrors `loadingOlder`: the bottom
      // sentinel can fire repeatedly, so drop overlapping same-cursor fetches.
      if (
        !conversationId ||
        !state.hasMoreAfter ||
        state.isStreaming ||
        state.loadingNewer
      ) {
        return
      }
      const newestId = lastMessageId(state.messages)
      if (!newestId) return

      set({ loadingNewer: true })
      try {
        const page = await ApiClient.Message.getHistory({
          id: conversationId,
          after: newestId,
          limit: MESSAGE_PAGE_SIZE,
        })
        if (get().conversation?.id !== conversationId) return
        set(s => ({
          messages: appendWindow(s.messages, page.messages),
          hasMoreAfter: page.has_more_after,
          loadingNewer: false,
        }))
        await get().computeForkPoints()
      } catch (error: any) {
        if (get().conversation?.id === conversationId) {
          set({
            error: error.message || 'Failed to load newer messages',
            loadingNewer: false,
          })
        }
      }
    },

    jumpToMessage: async (messageId: string): Promise<boolean> => {
      const conversationId = get().conversation?.id
      if (!conversationId) return false

      // Already loaded → no fetch needed; caller scrolls to it.
      if (get().messages.has(messageId)) return true

      try {
        const page = await ApiClient.Message.getHistory({
          id: conversationId,
          around: messageId,
          limit: MESSAGE_PAGE_SIZE,
        })
        if (get().conversation?.id !== conversationId) return false
        // Replace the window with the centered window.
        set({
          messages: toOrderedMap(page.messages),
          hasMoreBefore: page.has_more_before,
          hasMoreAfter: page.has_more_after,
          loadingOlder: false,
          loadingNewer: false,
        })
        await get().computeForkPoints()
        return get().messages.has(messageId)
      } catch (error: any) {
        if (get().conversation?.id === conversationId) {
          set({ error: error.message || 'Failed to jump to message' })
        }
        return false
      }
    },

    reconcileTail: async (conversationId: string) => {
      try {
        const page = await ApiClient.Message.getHistory({
          id: conversationId,
          limit: MESSAGE_PAGE_SIZE,
        })
        // Only apply to the still-open conversation.
        if (get().conversation?.id !== conversationId) return
        if (get().hasMoreAfter) {
          // The window is anchored MID-conversation (e.g. after an around=
          // jump), so the loaded slice does NOT abut the real tail — a merge
          // would splice the tail on after a gap. Snap to the tail instead.
          set({
            messages: toOrderedMap(page.messages),
            hasMoreBefore: page.has_more_before,
            hasMoreAfter: page.has_more_after,
            loadingOlder: false,
            loadingNewer: false,
          })
        } else {
          // Window already includes the tail: merge so loaded older pages stay
          // and the new turn appends at the bottom.
          set(s => ({
            messages: mergeTailWindow(s.messages, page.messages),
            hasMoreAfter: false,
          }))
        }
      } catch (error: any) {
        if (get().conversation?.id === conversationId) {
          set({ error: error.message || 'Failed to refresh messages' })
        }
      }
    },

    // ── Branch actions ─────────────────────────────────────────────────────

    loadBranches: async (conversationId: string) => {
      set({ branchesLoading: true })
      try {
        const branches = await ApiClient.Branch.list({ id: conversationId })

        // Seed branchForkLevels from the persisted fork_level on each branch.
        // This ensures computeForkPoints anchors the navigator correctly after page reload,
        // without relying on in-memory state that is lost on refresh.
        const branchForkLevels = new Map(
          branches.map(b => [
            b.id,
            (b.fork_level ?? 'user') as 'user' | 'assistant',
          ]),
        )

        set({ branches, branchForkLevels, branchesLoading: false })
        await get().computeForkPoints()
      } catch (err) {
        console.error('[Chat.store] Failed to load branches:', err)
        set({ branchesLoading: false })
      }
    },

    activateBranch: async (conversationId: string, branchId: string) => {
      await ApiClient.Branch.activate({
        id: conversationId,
        branch_id: branchId,
      })

      set(state => ({
        conversation: state.conversation
          ? { ...state.conversation, active_branch_id: branchId }
          : null,
      }))

      await get().loadMessages(conversationId)

      const { branches } = get()
      if (!branches.find(b => b.id === branchId)) {
        await get().loadBranches(conversationId)
      } else {
        await get().computeForkPoints()
      }
    },

    computeForkPoints: async () => {
      const state = get()
      const { branches, branchForkLevels } = state
      const conversation = state.conversation

      if (!conversation || branches.length <= 1) {
        set({ forkPoints: new Map() })
        return
      }

      const activeBranchId = conversation.active_branch_id
      const messages = [...state.messages.values()].sort(
        (a, b) =>
          new Date(a.created_at).getTime() - new Date(b.created_at).getTime(),
      )
      const messageIds = new Set(messages.map(m => m.id))

      const forkPoints = new Map<string, string[]>()

      // Group child branches by composite key: `${created_from_message_id}__${forkLevel}`.
      // A user message can be the fork origin for two independent sets of branches —
      // one from Regenerate ('assistant' level) and one from Edit ('user' level).
      // Grouping by both dimensions ensures each produces its own independent navigator.
      const forkGroups = new Map<string, string[]>()
      for (const branch of branches) {
        if (branch.created_from_message_id) {
          const forkLevel = branchForkLevels.get(branch.id) ?? 'user'
          const key = `${branch.created_from_message_id}__${forkLevel}`
          if (!forkGroups.has(key)) {
            forkGroups.set(key, [])
          }
          forkGroups.get(key)!.push(branch.id)
        }
      }

      const currentBranch = branches.find(b => b.id === activeBranchId)

      for (const [groupKey, childBranchIds] of forkGroups) {
        const separatorIdx = groupKey.lastIndexOf('__')
        const forkMsgId = groupKey.slice(0, separatorIdx)
        const forkLevel = groupKey.slice(separatorIdx + 2) as
          | 'user'
          | 'assistant'

        const firstChild = branches.find(b => b.id === childBranchIds[0])
        const parentBranchId = firstChild?.parent_branch_id

        const groupBranchIds = parentBranchId
          ? [parentBranchId, ...childBranchIds]
          : childBranchIds

        const groupBranches = groupBranchIds
          .map(id => branches.find(b => b.id === id))
          .filter(Boolean)
          .sort(
            (a, b) =>
              new Date(a!.created_at).getTime() -
              new Date(b!.created_at).getTime(),
          )
        const sortedGroupIds = groupBranches.map(b => b!.id)

        if (sortedGroupIds.length <= 1) continue

        let anchorMessageId: string | null = null

        if (activeBranchId === parentBranchId) {
          anchorMessageId = computeParentAnchor(
            forkMsgId,
            forkLevel,
            messages,
            messageIds,
          )
        } else if (
          activeBranchId &&
          childBranchIds.includes(activeBranchId) &&
          currentBranch
        ) {
          anchorMessageId = computeChildAnchor(
            activeBranchId,
            currentBranch.created_at,
            messages,
            branchForkLevels,
          )
        }

        if (anchorMessageId) {
          forkPoints.set(anchorMessageId, sortedGroupIds)
        }
      }

      set({ forkPoints })
    },

    trimMessagesToForkPoint: (forkMessageId: string) => {
      set(state => {
        const sorted = [...state.messages.values()].sort(
          (a, b) =>
            new Date(a.created_at).getTime() - new Date(b.created_at).getTime(),
        )
        const forkIndex = sorted.findIndex(m => m.id === forkMessageId)
        if (forkIndex === -1) return {}
        const newMessages = new Map(state.messages)
        sorted.slice(forkIndex).forEach(m => newMessages.delete(m.id))
        return { messages: newMessages }
      })
    },

    captureBranchForkLevel: (branchId: string) => {
      const level = get().pendingBranchForkLevel
      const newLevels = new Map(get().branchForkLevels)
      newLevels.set(branchId, level ?? 'user')
      set({ branchForkLevels: newLevels, pendingBranchForkLevel: null })
    },

    clearPendingBranch: () => {
      set({
        pendingBranchFromMessageId: null,
        pendingBranchForkLevel: null,
        editingMessage: null,
      })
    },

    startEditMessage: async (messageId: string) => {
      const message = get().messages.get(messageId)
      if (!message || message.role !== 'user') return

      // Trim messages to fork point so UI shows clean branch base immediately
      get().trimMessagesToForkPoint(messageId)

      // Set editing state — extensions subscribe to editingMessage via
      // useChatStore.subscribe() in their initialize() hooks
      set({
        editingMessage: message,
        pendingBranchFromMessageId: messageId,
        pendingBranchForkLevel: 'user',
      })

      // Pre-fill text input with message text content
      const textContent = message.contents
        .filter(c => c.content_type === 'text')
        .map(c => (c.content as any).text as string)
        .join('')
      ;(get() as any).TextStore?.setText(textContent)
    },

    cancelEdit: async () => {
      // Capture the edited message id BEFORE clearing so we can restore its
      // neighborhood (not just the tail) when it was scrolled up mid-history.
      const editedId = get().editingMessage?.id

      // Clear text input first
      ;(get() as any).TextStore?.clearText()

      // Clear editing state — extensions react via their subscribe handlers
      set({
        editingMessage: null,
        pendingBranchFromMessageId: null,
        pendingBranchForkLevel: null,
      })

      // Restore what was trimmed by startEditMessage. If the edited message sat
      // in the middle of a long (lazy-loaded) history, restore the window
      // CENTERED on it (around=) rather than snapping to the tail; fall back to
      // the tail if it can't be located on the active branch.
      const conversationId = get().conversation?.id
      if (!conversationId) return
      if (editedId) {
        const ok = await get().jumpToMessage(editedId)
        if (!ok) await get().loadMessages(conversationId)
      } else {
        await get().loadMessages(conversationId)
      }
    },

    startRegenerateMessage: async (assistantMessageId: string) => {
      const sorted = [...get().messages.values()].sort(
        (a, b) =>
          new Date(a.created_at).getTime() - new Date(b.created_at).getTime(),
      )

      const currentIndex = sorted.findIndex(m => m.id === assistantMessageId)
      if (currentIndex <= 0) return

      let precedingUserMsg = null
      for (let i = currentIndex - 1; i >= 0; i--) {
        if (sorted[i].role === 'user') {
          precedingUserMsg = sorted[i]
          break
        }
      }

      if (!precedingUserMsg) return

      const userText = (() => {
        for (const content of precedingUserMsg.contents) {
          const data = content.content as any
          if (data?.type === 'text' && typeof data.text === 'string') {
            return data.text
          }
        }
        return ''
      })()

      // Fan out content-block restoration to every extension —
      // each filters by its own content_type and rehydrates its
      // store accordingly (file restores `file_attachment` blocks
      // into its selectedFiles buffer; future extensions can do the
      // same for their content types). Chat itself stays
      // content-type-agnostic.
      const { chatExtensionRegistry } = await import(
        '@/modules/chat/core/extensions'
      )
      await chatExtensionRegistry.onMessageEditRestore(
        precedingUserMsg.contents,
      )

      // Pre-fill text input with the original user message text. Skip only
      // the pre-fill when the preceding user message is attachment-only (no
      // text) — the regeneration itself must still proceed below.
      if (userText) (get() as any).TextStore?.setText(userText)

      // Mark as assistant-level fork so computeForkPoints anchors the
      // navigator at the assistant bubble on both parent and child branches
      set({
        pendingBranchForkLevel: 'assistant',
        pendingBranchFromMessageId: precedingUserMsg.id,
      })

      // Trim the user message and everything after so the UI shows a clean
      // state during streaming
      get().trimMessagesToForkPoint(precedingUserMsg.id)

      await get().sendMessage()
    },

    // ── Send message with SSE streaming ───────────────────────────────────

    // Route ONE live generation frame (from the per-user chat-token stream,
    // tagged with its conversation) into the open conversation's streaming
    // state. Runs on EVERY device — whether this device sent the message or
    // another did — so a device with the conversation open renders live tokens
    // regardless of origin. The server already scopes frames to the open
    // conversation; the `conversationId` guards below drop a straggler that
    // lands just after a switch. This is the relocated started/content/
    // complete/error assembly that used to live inline in `sendMessage`.
    applyStreamFrame: async (conversationId: string, event: any) => {
      const type = event?.type

      // Mark the OPEN conversation as streaming on started/content. Critical for
      // a RECEIVING device (one watching a generation another device started) —
      // it never went through `sendMessage`, so without this its "generating"
      // affordance never shows AND the reconnect/`reloadOpen` `isStreaming`
      // guard wouldn't protect its live buffer from a refetch. Also capture the
      // assistant message id (from content frames) so a receiver can stop too.
      if (
        (type === 'started' || type === 'content') &&
        get().conversation?.id === conversationId
      ) {
        if (event?.message_id && !get().streamingMessageId) {
          set({ isStreaming: true, streamingMessageId: event.message_id })
        } else {
          set({ isStreaming: true })
        }
      }

      if (type === 'started') {
        // Drop a straggler that lands just after a switch: everything below
        // MUTATES the open conversation (branch id, temp-swap, extension stream
        // state), so applying an off-screen frame would corrupt the open view.
        if (get().conversation?.id !== conversationId) return

        await chatExtensionRegistry.onStreamStart()

        // Detect branch change (e.g. edit/regenerate created a new branch).
        const currentBranchId = get().conversation?.active_branch_id
        if (event.branch_id && event.branch_id !== currentBranchId) {
          set(state => ({
            conversation: state.conversation
              ? { ...state.conversation, active_branch_id: event.branch_id }
              : null,
            branchChangedDuringStream: true,
          }))
          get().captureBranchForkLevel(event.branch_id)
          const conversation = get().conversation
          if (conversation) await get().loadBranches(conversation.id)
        }

        const sseEvent: SSEEvent = { event_type: 'started', data: event }
        const handled = await chatExtensionRegistry.handleSSEEvent(sseEvent, get, set)
        if (handled) return

        const state = get()
        if (event.user_message_id && state.tempUserMessageId) {
          // This device sent the message: reconcile the optimistic temp id.
          // (Idempotent: the POST response may have already done this swap.)
          const tempMessage = state.messages.get(state.tempUserMessageId)
          if (tempMessage) {
            set(state => {
              const newMessages = new Map(state.messages)
              newMessages.delete(state.tempUserMessageId!)
              newMessages.set(event.user_message_id, {
                ...tempMessage,
                id: event.user_message_id,
                contents: tempMessage.contents.map(content => ({
                  ...content,
                  message_id: event.user_message_id,
                })),
              })
              return { messages: newMessages, tempUserMessageId: null }
            })
          }
        } else if (
          event.user_message_id &&
          conversationId === get().conversation?.id &&
          !get().messages.has(event.user_message_id)
        ) {
          // Receiving device (never had a temp): another device sent this
          // message. Merge the tail so the user bubble renders before the
          // assistant tokens fill in, without discarding loaded older pages.
          // Covers a catch-up replay too.
          await get().reconcileTail(conversationId)
        }
        return
      }

      if (type === 'content') {
        // Drop a straggler before any side-effect (extension dispatch included),
        // so an off-screen frame can't drive extension state for a conversation
        // we've already switched away from.
        if (get().conversation?.id !== conversationId) return

        const data = event
        const sseEvent: SSEEvent = { event_type: 'content', data }
        const handled = await chatExtensionRegistry.handleSSEEvent(sseEvent, get, set)
        if (handled) return

        const state = get()
        if (data.content && Array.isArray(data.content)) {
          if (!state.streamingMessage && data.content.length > 0) {
            const placeholderId = data.message_id || `streaming-${Date.now()}`
            const placeholder: MessageWithContent = {
              id: placeholderId,
              role: 'assistant',
              contents: [],
              originated_from_id: '',
              edit_count: 0,
              created_at: new Date().toISOString(),
            }
            set(state => {
              const newMessages = new Map(state.messages)
              newMessages.set(placeholder.id, placeholder)
              return { streamingMessage: placeholder, messages: newMessages }
            })
          }

          for (const block of data.content) {
            if (block.type === 'text_delta') {
              const currentState = get()
              const hasTextContent =
                currentState.streamingMessage?.contents.some(
                  c =>
                    c.content_type === 'text' ||
                    (c.content as any)?.type === 'text',
                ) ?? false

              if (!currentState.streamingMessage || !hasTextContent) {
                const messageId =
                  currentState.streamingMessage?.id ||
                  data.message_id ||
                  `streaming-${Date.now()}`
                const initialContent =
                  await chatExtensionRegistry.provideStreamingContent(
                    'text',
                    block.delta,
                  )
                if (initialContent) {
                  const baseMessage = currentState.streamingMessage ?? {
                    id: messageId,
                    role: 'assistant' as const,
                    contents: [],
                    originated_from_id: '',
                    edit_count: 0,
                    created_at: new Date().toISOString(),
                  }
                  const newContent = {
                    ...initialContent,
                    id: `${messageId}-content-${baseMessage.contents.length}`,
                    message_id: messageId,
                    sequence_order: baseMessage.contents.length,
                  }
                  const newMessage: MessageWithContent = {
                    ...baseMessage,
                    id: messageId,
                    contents: [...baseMessage.contents, newContent],
                  }
                  set(state => {
                    const newMessages = new Map(state.messages)
                    newMessages.set(newMessage.id, newMessage)
                    return {
                      streamingMessage: newMessage,
                      messages: newMessages,
                    }
                  })
                }
              } else {
                const delta = block.delta || ''
                const incomingMessageId = data.message_id
                set(currentState => {
                  if (!currentState.streamingMessage) return {}
                  const stableId = currentState.streamingMessage.id
                  const dbId = incomingMessageId || stableId
                  const existingContents =
                    currentState.streamingMessage.contents
                  const lastBlock =
                    existingContents[existingContents.length - 1]
                  const lastIsText =
                    !!lastBlock &&
                    (lastBlock.content_type === 'text' ||
                      (lastBlock.content as any)?.type === 'text')

                  let updatedContents: MessageContent[]
                  if (lastIsText) {
                    const currentText = (lastBlock.content as any)?.text || ''
                    updatedContents = [...existingContents]
                    updatedContents[existingContents.length - 1] = {
                      ...lastBlock,
                      content: {
                        ...lastBlock.content,
                        text: currentText + delta,
                      } as any,
                    }
                  } else {
                    const now = new Date().toISOString()
                    updatedContents = [
                      ...existingContents,
                      {
                        id: `${stableId}-content-${existingContents.length}`,
                        message_id: dbId,
                        content_type: 'text',
                        content: { type: 'text', text: delta } as any,
                        sequence_order: existingContents.length,
                        created_at: now,
                        updated_at: now,
                      },
                    ]
                  }

                  const updatedMessage: MessageWithContent = {
                    ...currentState.streamingMessage,
                    contents: updatedContents.map(c => ({
                      ...c,
                      message_id: dbId,
                    })),
                  }
                  const newMessages = new Map(currentState.messages)
                  newMessages.set(stableId, updatedMessage)
                  return {
                    streamingMessage: updatedMessage,
                    messages: newMessages,
                  }
                })
              }
            }
          }
        }
        return
      }

      if (type === 'complete') {
        const sseEvent: SSEEvent = { event_type: 'complete', data: event }
        const handled = await chatExtensionRegistry.handleSSEEvent(sseEvent, get, set)
        if (handled) return

        const { streamingMessage } = get()
        const isOnOriginalConversation =
          get().conversation?.id === conversationId

        set(state => {
          const newMessages = new Map(state.messages)
          if (state.streamingMessage) {
            newMessages.delete(state.streamingMessage.id)
          }
          return {
            isStreaming: false,
            sending: false,
            streamingMessage: null,
            streamingAbortController: null,
            streamingMessageId: null,
            messages: newMessages,
          }
        })

        if (isOnOriginalConversation) {
          if (streamingMessage) {
            await chatExtensionRegistry.afterStreamComplete(streamingMessage)
          }
          // Capture BEFORE clearing: an edit/regenerate created a NEW branch
          // during this stream, so the loaded window still holds the old
          // branch's prefix — cursors/merge would be inconsistent. Reset to the
          // new branch's tail instead of merging.
          const branchChanged = get().branchChangedDuringStream
          set({ branchChangedDuringStream: false })
          const conversation = get().conversation
          if (conversation) {
            if (branchChanged) {
              await get().loadMessages(conversation.id)
            } else {
              // Merge the finalized tail into the window WITHOUT discarding any
              // older pages the user scrolled up to load (DEC-6). The sidebar
              // message_count self-heals via the `Conversation` sync the backend
              // emits on turn completion (streaming.rs), so we no longer emit an
              // optimistic `messageCountChanged` here — under lazy-load
              // `messages.size` is only the loaded window, not the true total.
              await get().reconcileTail(conversation.id)
            }
          }
          await get().computeForkPoints()
        } else {
          get().clearConversationCache(conversationId)
        }
        return
      }

      if (type === 'error') {
        const streamError = new Error(event.message || 'Stream error')
        await chatExtensionRegistry.onStreamError(streamError)
        const sseEvent: SSEEvent = { event_type: 'error', data: event }
        await chatExtensionRegistry.handleSSEEvent(sseEvent, get, set)

        if (get().conversation?.id !== conversationId) {
          set({
            isStreaming: false,
            sending: false,
            streamingMessage: null,
            streamingAbortController: null,
            streamingMessageId: null,
          })
          get().clearConversationCache(conversationId)
          return
        }

        const state = get()
        if (state.tempUserMessageId) {
          set(state => {
            const newMessages = new Map(state.messages)
            newMessages.delete(state.tempUserMessageId!)
            return {
              messages: newMessages,
              tempUserMessageId: null,
              error: event.message || 'Stream error',
              isStreaming: false,
              sending: false,
              streamingMessage: null,
              streamingAbortController: null,
              streamingMessageId: null,
            }
          })
        } else {
          set({
            error: event.message || 'Stream error',
            isStreaming: false,
            sending: false,
            streamingMessage: null,
            streamingAbortController: null,
            streamingMessageId: null,
          })
        }
        return
      }

      // Extension events (titleUpdated, mcpToolStart/Complete/Progress,
      // mcpApprovalRequired, mcpElicitationRequired, artifactCreated, …) —
      // route through the extension registry exactly as the old inline
      // `default` SSE handler did. The backend forwards these onto the
      // chat-token stream alongside content frames.
      const sseEvent: SSEEvent = { event_type: type, data: event }
      await chatExtensionRegistry.handleSSEEvent(sseEvent, get, set)
    },

    sendMessage: async () => {
      let { conversation } = get()

      const beforeResult = await chatExtensionRegistry.beforeSendMessage()

      if (beforeResult.cancel) {
        console.log('[Chat.store] Message send cancelled by extension')
        throw new Error(
          beforeResult.errorMessage || 'Message send was cancelled',
        )
      }

      // Collect all request fields from extensions
      const allRequestFields =
        await chatExtensionRegistry.composeRequestFields()

      // Inject branching fields directly (moved from branching extension)
      const pendingBranchFromMessageId = get().pendingBranchFromMessageId
      if (pendingBranchFromMessageId) {
        allRequestFields.create_branch_from_message_id =
          pendingBranchFromMessageId
        allRequestFields.fork_level = get().pendingBranchForkLevel ?? 'user'
      }

      if (!conversation) {
        // Deferred emission: extensions get to mutate the freshly
        // created conversation BEFORE subscribers see the event.
        // The `afterCreateConversation` hook can return a replacement
        // shape; chat adopts it and emits the post-hook conversation.
        conversation = await get().createConversation(
          undefined,
          allRequestFields.model_id as string | undefined,
          /* emitCreated */ false,
        )
        const afterHook =
          await chatExtensionRegistry.afterCreateConversation(conversation)
        if (afterHook !== conversation) {
          conversation = afterHook
          set({ conversation })
        }
        const { Stores } = await import('@/core/stores')
        await Stores.EventBus.emit({
          type: 'conversation.created',
          data: { conversation },
        })
        await chatExtensionRegistry.initialize()
        await chatExtensionRegistry.onConversationLoad(conversation)
      }

      set({ sending: true, isStreaming: true, error: null })

      // If the window is anchored MID-conversation (after an around=/find/
      // deep-link jump, so `hasMoreAfter` is true), the loaded slice does not
      // abut the real tail. Snap to the tail first so the new turn's optimistic
      // bubble appends at the actual end instead of after a gap of unloaded
      // messages (reconciled again on `complete`, but this fixes the optimistic
      // render order too).
      if (get().hasMoreAfter) {
        await get().loadMessages(conversation.id)
      }

      const userContents = await chatExtensionRegistry.provideUserContent(
        (allRequestFields.content as string) || '',
        allRequestFields,
      )

      const tempUserMessage: MessageWithContent = {
        id: `temp-${Date.now()}`,
        role: 'user',
        contents: userContents,
        originated_from_id: '',
        edit_count: 0,
        created_at: new Date().toISOString(),
      }

      set(state => {
        const newMessages = new Map(state.messages)
        newMessages.set(tempUserMessage.id, tempUserMessage)
        return {
          messages: newMessages,
          tempUserMessageId: tempUserMessage.id,
        }
      })

      try {
        // Subscribe this device's token stream to the (possibly just-created)
        // conversation BEFORE kicking off generation, so it receives all of its
        // own tokens. Idempotent/deduped for an already-open conversation.
        await get().chatStreamClient?.setActiveConversation(conversation.id)

        // Fire-and-forget: the assistant reply streams over the chat-token
        // stream (applied by `applyStreamFrame` via the `chat:token` router),
        // not this response.
        const { user_message_id, assistant_message_id } =
          await ApiClient.Message.send({
            id: conversation.id,
            branch_id: conversation.active_branch_id || '',
            ...allRequestFields,
          } as any)

        // Remember the assistant message so the stop button can address it.
        set({ streamingMessageId: assistant_message_id })

        // Reconcile the optimistic temp user message to its real id. The
        // `started` frame may also do this swap; both are idempotent.
        if (user_message_id && get().tempUserMessageId) {
          const tempId = get().tempUserMessageId!
          const tempMessage = get().messages.get(tempId)
          if (tempMessage) {
            set(state => {
              const newMessages = new Map(state.messages)
              newMessages.delete(tempId)
              newMessages.set(user_message_id, {
                ...tempMessage,
                id: user_message_id,
                contents: tempMessage.contents.map(c => ({
                  ...c,
                  message_id: user_message_id,
                })),
              })
              return { messages: newMessages, tempUserMessageId: null }
            })
          }
        }

        await chatExtensionRegistry.onMessageSent()
        get().clearPendingBranch()
        set({ sending: false })
      } catch (error: any) {
        const isAborted = error instanceof Error && error.name === 'AbortError'

        if (!isAborted) {
          await chatExtensionRegistry.onStreamError(
            error instanceof Error
              ? error
              : new Error(error.message || 'Failed to send message'),
          )
        }

        const state = get()
        const baseUpdate = {
          error: isAborted ? null : error.message || 'Failed to send message',
          sending: false,
          isStreaming: false,
          streamingMessage: null,
          streamingAbortController: null,
          streamingMessageId: null,
        }

        if (state.tempUserMessageId) {
          set(state => {
            const newMessages = new Map(state.messages)
            newMessages.delete(state.tempUserMessageId!)
            return {
              messages: newMessages,
              tempUserMessageId: null,
              ...baseUpdate,
            }
          })
        } else {
          set(baseUpdate)
        }

        if (isAborted) {
          const conversation = get().conversation
          if (conversation) {
            await get().loadMessages(conversation.id)
          }
        }
      }
    },

    updateConversation: async (updates: { title?: string }) => {
      const { conversation } = get()
      if (!conversation) {
        set({ error: 'No active conversation' })
        return
      }

      try {
        await ApiClient.Conversation.update({
          id: conversation.id,
          ...updates,
        })

        set(state => ({
          conversation: state.conversation
            ? { ...state.conversation, ...updates }
            : null,
        }))

        if (updates.title !== undefined) {
          const { Stores } = await import('@/core/stores')
          await Stores.EventBus.emit({
            type: 'conversation.titleUpdated',
            data: {
              conversationId: conversation.id,
              title: updates.title,
            },
          })
        }
      } catch (error: any) {
        set({
          error: error.message || 'Failed to update conversation',
        })
        throw error
      }
    },

    clearError: () => set({ error: null }),

    stopStreaming: () => {
      // Generation runs server-side (detached); cancel it via the stop
      // endpoint. The detached task emits a `complete` (cancelled) frame which
      // `applyStreamFrame` then reconciles.
      const conversation = get().conversation
      const messageId = get().streamingMessageId
      if (conversation && messageId) {
        void ApiClient.Message.stopGeneration({
          conversation_id: conversation.id,
          assistant_message_id: messageId,
        })
      }
    },

    displayInRightPanel: <T extends PanelType>(entry: RightPanelTab<T>) => {
      set(state => {
        const exists = state.rightPanel.tabs.some(t => t.id === entry.id)
        if (exists) {
          return {
            rightPanel: {
              ...state.rightPanel,
              activeId: entry.id,
              mobileDrawerOpen: true,
            },
          }
        }
        return {
          rightPanel: {
            ...state.rightPanel,
            tabs: [...state.rightPanel.tabs, entry as RightPanelTab],
            activeId: entry.id,
            mobileDrawerOpen: true,
          },
        }
      })
      const { rightPanel, conversation } = get()
      if (conversation) {
        savePanelSnapshotForConversation(
          conversation.id,
          rightPanel.tabs,
          rightPanel.activeId,
        )
      }
    },

    updateRightPanelTab: <T extends PanelType>(id: string, data: PanelRendererMap[T]) => {
      set(state => {
        const idx = state.rightPanel.tabs.findIndex(t => t.id === id)
        if (idx === -1) return state
        const tabs = state.rightPanel.tabs.slice()
        tabs[idx] = { ...tabs[idx], data: data as RightPanelTab['data'] }
        return { rightPanel: { ...state.rightPanel, tabs } }
      })
      const { rightPanel, conversation } = get()
      if (conversation) {
        savePanelSnapshotForConversation(
          conversation.id,
          rightPanel.tabs,
          rightPanel.activeId,
        )
      }
    },

    setActiveRightPanelTab: (id: string) => {
      set(state => {
        if (!state.rightPanel.tabs.some(t => t.id === id)) return state
        return { rightPanel: { ...state.rightPanel, activeId: id } }
      })
    },

    closeRightPanelTab: (id: string) => {
      set(state => {
        const tabs = state.rightPanel.tabs.filter(t => t.id !== id)
        let activeId = state.rightPanel.activeId
        if (activeId === id) {
          const closedIndex = state.rightPanel.tabs.findIndex(t => t.id === id)
          const next = tabs[closedIndex] ?? tabs[closedIndex - 1] ?? null
          activeId = next?.id ?? null
        }
        const mobileDrawerOpen =
          tabs.length > 0 ? state.rightPanel.mobileDrawerOpen : false
        return {
          rightPanel: { ...state.rightPanel, tabs, activeId, mobileDrawerOpen },
        }
      })
      const { rightPanel, conversation } = get()
      if (conversation) {
        savePanelSnapshotForConversation(
          conversation.id,
          rightPanel.tabs,
          rightPanel.activeId,
        )
      }
    },

    closeAllRightPanelTabs: () => {
      set(state => ({
        rightPanel: {
          ...state.rightPanel,
          tabs: [],
          activeId: null,
          mobileDrawerOpen: false,
        },
      }))
      const { conversation } = get()
      if (conversation) {
        savePanelSnapshotForConversation(conversation.id, [], null)
      }
    },

    closeMobileDrawer: () => {
      set(state => ({
        rightPanel: { ...state.rightPanel, mobileDrawerOpen: false },
      }))
    },

    setRightPanelWidth: (width: number) => {
      set(state => ({ rightPanel: { ...state.rightPanel, panelWidth: width } }))
    },

    reset: async () => {
      // Leaving for a new chat: stop receiving any conversation's tokens.
      void get().chatStreamClient?.setActiveConversation(null)
      const { conversation } = get()
      if (conversation) {
        get().saveConversationState(conversation.id)
        get().scheduleCacheClear(conversation.id)

        // Save outgoing conversation's panel tabs to localStorage before clearing
        const { rightPanel } = get()
        savePanelSnapshotForConversation(
          conversation.id,
          rightPanel.tabs,
          rightPanel.activeId,
        )

        await chatExtensionRegistry.cleanup()
      }

      set(state => ({
        conversation: null,
        messages: new Map<string, MessageWithContent>(),
        loading: false,
        loadingConversationId: null,
        sending: false,
        isStreaming: false,
        error: null,
        hasMoreBefore: false,
        hasMoreAfter: false,
        loadingOlder: false,
        loadingNewer: false,
        streamingMessage: null,
        tempUserMessageId: null,
        streamingMessageId: null,
        branches: [],
        branchesLoading: false,
        pendingBranchFromMessageId: null,
        pendingBranchForkLevel: null,
        branchForkLevels: new Map(),
        branchChangedDuringStream: false,
        forkPoints: new Map(),
        editingMessage: null,
        rightPanel: {
          ...state.rightPanel,
          tabs: [],
          activeId: null,
          mobileDrawerOpen: false,
        },
      }))
    },

    // ── Lifecycle methods ──────────────────────────────────────────────────

    }
  },
  init: ({
    set,
    get: getRaw,
    on,
    onCleanup,
  }: StoreInitCtx<typeof chatInitialState>) => {
    const get = getRaw as () => ChatState

    // Idempotent: `__init__.__store__` can be invoked more than once per instance
    // (a local pane self-inits via `.use()`, and the `Stores.Chat` proxy's lazy
    // init check may also fire it for the focused pane). Bail if this instance
    // already created its client, so we never stack a second client / auth
    // subscription.
    if (get().chatStreamClient) return

    // Per-instance stream/serialization state (formerly module singletons). Each
    // pane serializes its own frame-apply chain, throttles its own resync, and
    // owns its own stream client — so panes never couple (ITEM-6).
    let frameApplyTail: Promise<void> = Promise.resolve()
    let lastChatResyncAt = 0
    const streamClient = createChatStreamClient()
    set({ chatStreamClient: streamClient })

    // Give THIS instance its own extension-store instances (e.g. the composer
    // `TextStore`) so split panes don't share one. Idempotent — the primary's
    // register-time seed is left in place, so single-pane is unchanged (ITEM-4/5).
    chatExtensionRegistry.injectExtensionStores(
      get() as unknown as Record<string, unknown>,
    )

    void (async () => {
        // Cross-device sync: when the currently-OPEN conversation changed on
        // another device (a completed message turn, rename, branch switch,
        // edit/delete), refetch its messages + branches. Skip while we're
        // streaming locally — the live stream is authoritative and reconciles
        // on `complete`, and a refetch mid-stream would clobber the buffer.
        const reloadOpen = async (id: string) => {
          const state = get()
          if (state.conversation?.id !== id || state.isStreaming) return
          // Capture the active branch BEFORE refreshing metadata so we can tell
          // whether a remote change switched branches (→ reset the window) vs a
          // same-branch change like a new turn / rename (→ merge the tail and
          // preserve the user's scrolled-up older pages).
          const prevBranchId = state.conversation?.active_branch_id
          // Refresh conversation METADATA too (title/model/branch) — a remote
          // rename or auto-title only reaches the open view this way (the live
          // token stream no longer carries titleUpdated to non-senders).
          try {
            const conv = await ApiClient.Conversation.get({ id })
            if (get().conversation?.id === id) set({ conversation: conv })
          } catch {
            // fall through to message/branch reload
          }
          if (get().conversation?.id !== id || get().isStreaming) return
          const branchChanged =
            get().conversation?.active_branch_id !== prevBranchId
          if (branchChanged) {
            // Different branch path → cursors are invalid; reset to its tail.
            await get().loadMessages(id)
          } else {
            await get().reconcileTail(id)
          }
          await get().loadBranches(id)
          await get().computeForkPoints()
        }

        // Debounced reconnect resync (a flapping stream must not storm refetch).
        // Shared by both stream reconnects (sync + chat-token): both ultimately
        // do the same idempotent open-conversation refetch, so a suppressed
        // duplicate within the window loses nothing.
        const resyncOpen = () => {
          const id = get().conversation?.id
          if (!id) return
          const now = Date.now()
          if (now - lastChatResyncAt < CHAT_RESYNC_MIN_INTERVAL_MS) return
          lastChatResyncAt = now
          void reloadOpen(id)
        }

        on(
          'sync:conversation',
          event => {
            if (event.data.action === 'delete') {
              // A remote device deleted this conversation. If it's the one open
              // here, clear it — otherwise the store keeps pointing at a dead id
              // (sends would 404 and we'd keep subscribing to it). The list store
              // drops it from the sidebar separately.
              if (get().conversation?.id === event.data.id) get().reset()
              return
            }
            void reloadOpen(event.data.id)
          },
        )

        on('sync:reconnect', () => resyncOpen())

        // Live chat-token stream lifecycle for THIS instance's own client. Start
        // on auth, restart on user-switch, stop on logout — mirrors core/sync's
        // index but per-instance. The auth subscription + the client are torn
        // down in `onCleanup`, so a re-init (or a pane unmount) never stacks
        // subscribers or leaks a connection.
        const { useAuthStore } = await import('@/modules/auth/Auth.store')
        let currentUserId = useAuthStore.getState().user?.id
        const applyAuth = (userId: string | undefined) => {
          streamClient.stop()
          if (userId) streamClient.start()
        }
        applyAuth(currentUserId)
        const unsubscribeAuth = useAuthStore.subscribe(state => {
          const id = state.user?.id
          if (id === currentUserId) return
          currentUserId = id
          applyAuth(id)
        })
        onCleanup(unsubscribeAuth)

        // Inbound: route each live generation frame to the open conversation.
        // Fires on EVERY device (sender or receiver) — whichever has the
        // conversation open renders live tokens. SERIALIZED via a per-instance
        // tail promise: applyStreamFrame is async (awaits extension hooks /
        // loadMessages), so concurrent invocations would interleave and corrupt
        // streamingMessage. `applyStreamFrame` filters by conversation id, so a
        // frame for another pane's conversation is a cheap no-op here.
        on('chat:token', event => {
          frameApplyTail = frameApplyTail
            .then(() =>
              get().applyStreamFrame(
                event.data.conversation_id,
                event.data.event,
              ),
            )
            .catch(err => console.error('[chat:token] apply failed', err))
        })

        // On stream (re)connect the server replays the reply-so-far (catch-up);
        // also reconcile the open conversation from the DB (debounced).
        on('chat:stream-reconnect', () => resyncOpen())
    })()
    onCleanup(() => {
      console.log('[Chat.store] Destroying - cleaning up resources')

      // Stop this instance's own stream client (its own SSE connection). The
      // `ctx.on` subscriptions + the auth unsubscribe are torn down by the
      // store-kit builder's per-group cleanup — NO manual
      // `removeGroupListeners('Chat')` here: for a split pane that would wipe the
      // PRIMARY pane's ('Chat'-group) listeners too.
      streamClient.stop()

      const state = get()

      // Abort any in-flight streaming fetch BEFORE the rest of teardown.
      // Without this, when the user navigates away mid-stream and the
      // proxy refTracker schedules destruction (5s grace), the SSE
      // fetch keeps running. On re-init a SECOND parallel fetch is
      // spawned and the abandoned one's set() callbacks execute
      // against a frozen state. (audit 09 B-1)
      if (state.streamingAbortController) {
        state.streamingAbortController.abort()
      }

      for (const [conversationId, timer] of state.cacheClearTimers.entries()) {
        clearTimeout(timer)
        console.log(
          `[Chat.store] Cleared pending timer for conversation: ${conversationId}`,
        )
      }

      if (state.conversation) {
        get().saveConversationState(state.conversation.id)

        chatExtensionRegistry
          .cleanup()
          .catch(error =>
            console.error('[Chat.store] Extension cleanup failed:', error),
          )
      }

      state.conversationStateCache.clear()
      state.cacheClearTimers.clear()
      useMessageViewStateStore.getState().resetViewState()

      console.log('[Chat.store] Destroyed successfully')
    })
  },
}

/** Pane 0 — the eager primary chat store (a global singleton). Single-pane chat
 *  runs entirely on this instance, so behaviour is unchanged. The `Stores.Chat`
 *  bridge (chatBridge.ts) forwards to whichever pane is focused, defaulting here. */
export const Chat = defineStore('Chat', chatStoreConfig)

/** Per-pane chat store for additional split panes (ITEM-2). Same config as the
 *  primary; each `.use()` / `.create()` is an independent instance with its own
 *  EventBus group. */
export const ChatPaneStore = defineLocalStore(chatStoreConfig)

export const useChatStore = Chat.store
