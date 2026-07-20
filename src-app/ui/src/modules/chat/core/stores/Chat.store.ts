import { type ComponentType, memo, type ReactNode } from 'react'
import {
  defineLocalStore,
  defineStore,
  type StoreInitCtx,
  type StoreSet,
} from '@ziee/framework/store-kit'
import { useMessageViewStateStore } from '@/modules/chat/core/stores/MessageViewState.store'
import { ApiClient } from '@/api-client'
import type { Branch, Conversation, MessageWithContent } from '@/api-client/types'

import {
  type ChatStreamClient,
  createChatStreamClient,
} from '@/modules/chat/core/stream/ChatStreamClient'


import { chatExtensionRegistry } from '@/modules/chat/extensions'

/** Default page size for a message-history window (mirrors the backend default). */
export const MESSAGE_PAGE_SIZE = 30

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

export function loadAllPanelSnapshots(): Record<string, ConversationPanelSnapshot> {
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
export function touchPanelSnapshot(conversationId: string): void {
  const all = loadAllPanelSnapshots()
  const snap = all[conversationId]
  if (!snap) return
  snap.lastAccessedAt = Date.now()
  evictStaleSnapshots(all)
  saveAllPanelSnapshots(all)
}

export function savePanelSnapshotForConversation(
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
export function rehydrateTabs(persisted: RightPanelTab[]): RightPanelTab[] {
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

export interface ChatState {
  // Data
  conversation: Conversation | null
  messages: Map<string, MessageWithContent>

  // Loading states
  loading: boolean
  loadingConversationId: string | null
  sending: boolean
  isStreaming: boolean
  error: string | null
  /** HTTP status of the last failed conversation load (404/403) — see initial state. */
  lastLoadErrorStatus: number | null
  /** The last turn ended via cancel / stream-error / abort (a partial, not a
   *  genuine empty completion). Reset when a new send starts. Consumed by the
   *  message renderer to suppress the empty-completion notice on interrupted
   *  turns. Transient live state (not snapshotted / not persisted). */
  lastTurnInterrupted: boolean
  /** True only for the sub-second window between a turn's `complete` frame and
   *  the persisted tail being swapped in on-screen. Suppresses the
   *  empty-completion notice so a transient empty/absent assistant frame during
   *  the streaming→persisted handoff can never flash it. Transient live state
   *  (not snapshotted / not persisted). */
  finalizingTurn: boolean

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
  loadConversationState: (conversationId: string) => Promise<boolean>
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
  /** This pane's extension runtime (ITEM-34); null on the single-pane primary. */
  extensionRuntime: import('../extensions/types').ExtensionLifecycle | null
  /** This pane's stable id (ITEM-32/37); null on the single-pane primary. */
  paneId: string | null
  /** Attach a per-pane extension runtime (called by ChatPaneProvider on mount). */
  attachExtensionRuntime: (
    runtime: import('../extensions/types').ExtensionLifecycle | null,
  ) => void
  /** Set this pane's stable id (called by ChatPaneProvider on mount). */
  setPaneId: (paneId: string | null) => void
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
    // HTTP status of the last failed conversation load (404 gone / 403 no-access),
    // so a split pane can move itself out of the workspace when its conversation
    // is deleted or access is revoked (ITEM-29). Null on success / transient error.
    lastLoadErrorStatus: null as number | null,
    lastTurnInterrupted: false,
    finalizingTurn: false,
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
    // This pane's extension runtime (ITEM-34). Attached by `ChatPaneProvider` on
    // mount so lifecycle/hooks bind to THIS pane's store + its own `initialized`
    // flag. Null on the single-pane primary store → falls back to the global
    // `chatExtensionRegistry` (which binds to the singleton = correct).
    extensionRuntime: null as import(
      '../extensions/types'
    ).ExtensionLifecycle | null,
    // This pane's stable id (ITEM-32/37), attached by ChatPaneProvider. Scopes
    // the composer buffer (per-pane files) + the new-chat sentinel keys (model /
    // assistant / MCP) so two new-chat panes don't share one selection. Null on
    // the single-pane primary → the shared/global key (byte-identical).
    paneId: null as string | null,
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

export type ChatInitialState = typeof chatInitialState
export type ChatSet = StoreSet<ChatInitialState>

const chatStoreConfig = {
  state: chatInitialState,
  lazyActions: {
    loadConversationState: () => import('./chat/actions/loadConversationState'),
    createConversation: () => import('./chat/actions/createConversation'),
    loadConversation: () => import('./chat/actions/loadConversation'),
    loadMessages: () => import('./chat/actions/loadMessages'),
    loadOlderMessages: () => import('./chat/actions/loadOlderMessages'),
    loadNewerMessages: () => import('./chat/actions/loadNewerMessages'),
    jumpToMessage: () => import('./chat/actions/jumpToMessage'),
    reconcileTail: () => import('./chat/actions/reconcileTail'),
    loadBranches: () => import('./chat/actions/loadBranches'),
    activateBranch: () => import('./chat/actions/activateBranch'),
    computeForkPoints: () => import('./chat/actions/computeForkPoints'),
    startEditMessage: () => import('./chat/actions/startEditMessage'),
    cancelEdit: () => import('./chat/actions/cancelEdit'),
    startRegenerateMessage: () => import('./chat/actions/startRegenerateMessage'),
    applyStreamFrame: () => import('./chat/actions/applyStreamFrame'),
    sendMessage: () => import('./chat/actions/sendMessage'),
    updateConversation: () => import('./chat/actions/updateConversation'),
    reset: () => import('./chat/actions/reset'),
  },
  actions: (
    set: StoreSet<typeof chatInitialState>,
    getRaw: () => typeof chatInitialState,
  ) => {
    const get = getRaw as () => ChatState
    return {

    /** Attach a per-pane extension runtime (ChatPaneProvider, on mount). */
    attachExtensionRuntime: (
      runtime: import('../extensions/types').ExtensionLifecycle | null,
    ) => {
      set({ extensionRuntime: runtime })
    },

    /** Set this pane's stable id (ChatPaneProvider, on mount). */
    setPaneId: (paneId: string | null) => {
      set({ paneId })
    },

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

    // Warm the lazy-action chunks OFF the first-render path. `init` runs at first
    // store access (during/after the initial render), and `.preload()` only kicks
    // off a background dynamic import — it never blocks paint. The render-critical
    // + streaming actions warm immediately so the first send/stream has no hop;
    // the rest warm on idle so they're ready for interaction without competing
    // with first paint. Chunks are module-global, so priming via the singleton
    // (`useChatStore`) warms them for every pane. Typed `.preload()` comes from
    // the inferred lazy-dispatcher type (ChatState declares plain fns).
    const cs = useChatStore.getState()
    void cs.loadConversation.preload()
    void cs.loadMessages.preload()
    void cs.loadConversationState.preload()
    void cs.reconcileTail.preload()
    void cs.applyStreamFrame.preload()
    void cs.sendMessage.preload()
    const warmIdle = (cb: () => void) => {
      if (typeof requestIdleCallback !== 'undefined') requestIdleCallback(cb)
      else setTimeout(cb, 200)
    }
    warmIdle(() => {
      const c = useChatStore.getState()
      void c.createConversation.preload()
      void c.updateConversation.preload()
      void c.loadOlderMessages.preload()
      void c.loadNewerMessages.preload()
      void c.jumpToMessage.preload()
      void c.loadBranches.preload()
      void c.activateBranch.preload()
      void c.computeForkPoints.preload()
      void c.startEditMessage.preload()
      void c.cancelEdit.preload()
      void c.startRegenerateMessage.preload()
      void c.reset.preload()
    })

    // Idempotent: `__init__.__store__` can be invoked more than once per instance
    // (a local pane self-inits via `.use()`, and the `Stores.Chat` proxy's lazy
    // init check may also fire it for the focused pane). Bail if this instance
    // already created its client, so we never stack a second client / auth
    // subscription.
    if (get().chatStreamClient) return

    // Per-init-lifecycle teardown flag. The async continuation below restarts the
    // stream client AFTER an `await`; under React StrictMode a pane's init runs
    // init#1 → destroy#1 → init#2 on the SAME api, so init#1's resumed tail must
    // NOT restart its already-stopped client (that would leak an SSE the final
    // teardown can't reach). `onCleanup` sets this true; the tail bails on it.
    let destroyed = false

    // Per-instance stream/serialization state (formerly module singletons). Each
    // pane serializes its own frame-apply chain, throttles its own resync, and
    // owns its own stream client — so panes never couple (ITEM-6).
    let frameApplyTail: Promise<void> = Promise.resolve()
    let lastChatResyncAt = 0
    // Reconnect handler is forward-declared (resyncOpen is defined in the async
    // IIFE below); the client only fires it after a genuine (re)connect, long
    // after that assignment lands.
    let onStreamReconnect: () => void = () => {}
    // Deliver THIS client's frames DIRECTLY to THIS pane's store (ITEM-35) rather
    // than the global `chat:token` bus, so two panes on the same conversation
    // never double-process. SERIALIZED via the per-instance tail promise
    // (applyStreamFrame is async → concurrent calls would corrupt streamingMessage).
    const streamClient = createChatStreamClient({
      onFrame: (conversationId, event) => {
        frameApplyTail = frameApplyTail
          .then(() => get().applyStreamFrame(conversationId, event))
          .catch((err) => console.error('[chat:token] apply failed', err))
      },
      onReconnect: () => onStreamReconnect(),
    })
    set({ chatStreamClient: streamClient })

    // Give THIS instance its own extension-store instances (e.g. the composer
    // `TextStore`) so split panes don't share one. Idempotent — the primary's
    // register-time seed is left in place, so single-pane is unchanged (ITEM-4/5).
    ;(get().extensionRuntime ?? chatExtensionRegistry).injectExtensionStores(
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
        // Wire this pane's stream client reconnect → its own debounced resync
        // (ITEM-35): only THIS pane refetches on its own reconnect, not all panes.
        onStreamReconnect = resyncOpen

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
        // Bail if this init lifecycle was torn down during the await (a StrictMode
        // destroy#1 landing between init#1's await and its resume) — otherwise we'd
        // restart a stopped client and register an orphaned auth sub that the
        // already-drained teardown will never reach (DRIFT-2.16).
        if (destroyed) return
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

        // Inbound frames + stream-reconnect are now delivered DIRECTLY to this
        // pane's client via the onFrame/onReconnect callbacks wired at client
        // creation (ITEM-35) — no global `chat:token` / `chat:stream-reconnect`
        // EventBus subscription, so a same-conversation split never double-applies.
    })()
    onCleanup(() => {
      console.log('[Chat.store] Destroying - cleaning up resources')

      // Stop this instance's own stream client (its own SSE connection). The
      // `ctx.on` subscriptions + the auth unsubscribe are torn down by the
      // store-kit builder's per-group cleanup — NO manual
      // `removeGroupListeners('Chat')` here: for a split pane that would wipe the
      // PRIMARY pane's ('Chat'-group) listeners too.
      // Mark this init lifecycle destroyed FIRST, so init#1's still-pending async
      // tail (under a StrictMode init#1→destroy#1→init#2 remount) bails at its
      // `if (destroyed) return` instead of restarting this client (DRIFT-2.16).
      destroyed = true
      streamClient.stop()

      // Null the client so a destroy→re-init cycle passes the init guard
      // (`if (get().chatStreamClient) return`, ~L2098). The SINGLETON primary's
      // STATE OBJECT survives destroy (defineStore creates it once; ref-count
      // destroy only tears down subscriptions), so without this a navigate-away >5s
      // (grace destroy) + return leaves the stopped client in state, the guard
      // early-returns, and streaming + sync never re-establish (DRIFT-2.15). Now
      // SAFE for pane instances too: the `destroyed` flag above stops init#1's tail
      // from restarting the orphaned client, so nulling lets init#2 fully
      // re-register the teardown + sync subscriptions instead of early-returning
      // with them dropped (the StrictMode teardown-drop FIX_ROUND-7 caught).
      set({ chatStreamClient: null })

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

        // Route through THIS pane's runtime (ITEM-34) so a pane's teardown cleans
        // up its OWN extension subscriptions, not the singleton's; single-pane
        // falls back to the global registry.
        ;(state.extensionRuntime ?? chatExtensionRegistry)
          .cleanup()
          .catch(error =>
            console.error('[Chat.store] Extension cleanup failed:', error),
          )
      }

      state.conversationStateCache.clear()
      state.cacheClearTimers.clear()
      // Scoped to this instance's own messages so tearing down one split pane
      // doesn't wipe another pane's live view state (ITEM-21).
      useMessageViewStateStore
        .getState()
        .resetViewState(Array.from(state.messages.keys()))

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
