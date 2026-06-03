import { create } from 'zustand'
import { subscribeWithSelector } from 'zustand/middleware'
import { ApiClient } from '@/api-client'
import { memo, type ComponentType, type ReactNode } from 'react'
import type {
  Branch,
  Conversation,
  MessageContent,
  MessageWithContent,
} from '@/api-client/types'
import { chatExtensionRegistry } from '@/modules/chat/extensions'
import type { SSEEvent, GenericSSEEvent } from '@/modules/chat/core/extensions/types'
import {
  computeParentAnchor,
  computeChildAnchor,
} from '@/modules/chat/core/utils/branchAnchor.utils'

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

// Module-level registry of panel renderers, populated by extensions.
const panelRendererRegistry = new Map<PanelType, PanelRenderer<PanelType>>()

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
    // ComponentType but TS can't see through PanelRendererMap[T] indexing,
    // so widen via unknown.
    component: memo(renderer.component) as unknown as ComponentType<PanelRendererMap[T]>,
  } as PanelRenderer<PanelType>)
}

/**
 * Resolve a tab's renderer to its component + icon. Returns null and warns
 * (in dev) if no renderer is registered for the tab's type — this typically
 * means the owning extension hasn't initialized yet, or the type was removed.
 */
export function resolvePanelRenderer(tab: RightPanelTab): {
  Component: ComponentType<PanelRendererMap[PanelType]>
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
const PANEL_TTL_MS = 30 * 24 * 60 * 60 * 1000  // 30 days

function loadAllPanelSnapshots(): Record<string, ConversationPanelSnapshot> {
  try {
    const raw = localStorage.getItem(PANEL_STORAGE_KEY)
    if (!raw) return {}
    return JSON.parse(raw) as Record<string, ConversationPanelSnapshot>
  } catch {
    return {}
  }
}

function saveAllPanelSnapshots(snapshots: Record<string, ConversationPanelSnapshot>): void {
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
function evictStaleSnapshots(snapshots: Record<string, ConversationPanelSnapshot>): void {
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
    all[conversationId] = { tabs, activeId: persistedActiveId, lastAccessedAt: Date.now() }
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
  loadMessages: (id: string) => Promise<void>
  sendMessage: () => Promise<void>
  updateConversation: (updates: {
    title?: string
  }) => Promise<void>
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
  stopStreaming: () => void

  // ── Right panel ───────────────────────────────────────────────────────────

  rightPanel: { panelWidth: number; tabs: RightPanelTab[]; activeId: string | null; mobileDrawerOpen: boolean }
  displayInRightPanel: <T extends PanelType>(entry: RightPanelTab<T>) => void
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

export const useChatStore = create<ChatState>()(
  subscribeWithSelector((set, get) => ({
      // ── Initial state ──────────────────────────────────────────────────────

      conversation: null,
      messages: new Map<string, MessageWithContent>(),
      loading: false,
      loadingConversationId: null,
      sending: false,
      isStreaming: false,
      error: null,
      streamingMessage: null,
      tempUserMessageId: null,
      streamingAbortController: null,

      conversationStateCache: new Map<string, ChatStateSnapshot>(),
      cacheClearTimers: new Map<string, NodeJS.Timeout>(),

      // Branch initial state
      branches: [],
      branchesLoading: false,
      pendingBranchFromMessageId: null,
      pendingBranchForkLevel: null,
      branchForkLevels: new Map(),
      branchChangedDuringStream: false,
      forkPoints: new Map(),
      editingMessage: null,

      // Right panel initial state
      rightPanel: { panelWidth: 440, tabs: [], activeId: null, mobileDrawerOpen: false },

      // ── Conversation state management ──────────────────────────────────────

      saveConversationState: (conversationId: string) => {
        const state = get()
        const snapshot: ChatStateSnapshot = {
          conversation: state.conversation,
          messages: new Map(state.messages),
          streamingMessage: state.streamingMessage,
          tempUserMessageId: state.tempUserMessageId,
          isStreaming: state.isStreaming,
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
        const currentConversation = get().conversation
        const loadingId = get().loadingConversationId

        if (currentConversation && currentConversation.id === id) {
          console.log(`[Chat.store] Conversation ${id} already loaded, skipping`)
          return
        }

        if (loadingId === id) {
          console.log(`[Chat.store] Conversation ${id} is already loading, skipping`)
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
          savePanelSnapshotForConversation(currentConversation.id, rightPanel.tabs, rightPanel.activeId)
          set(state => ({ rightPanel: { ...state.rightPanel, tabs: [], activeId: null, mobileDrawerOpen: false } }))

          await chatExtensionRegistry.cleanup()
          set({ isStreaming: false, sending: false, streamingMessage: null, tempUserMessageId: null, streamingAbortController: null })
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
              set(state => ({ rightPanel: { ...state.rightPanel, tabs, activeId: panelSnapshot.activeId } }))
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
              set(state => ({ rightPanel: { ...state.rightPanel, tabs, activeId: panelSnapshot.activeId } }))
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
          const messagesArray = await ApiClient.Message.getHistory({ id })
          set({
            messages: new Map(messagesArray.map(msg => [msg.id, msg])),
            loading: false,
          })
        } catch (error: any) {
          set({
            error: error.message || 'Failed to load messages',
            loading: false,
          })
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
            branches.map(b => [b.id, (b.fork_level ?? 'user') as 'user' | 'assistant'])
          )

          set({ branches, branchForkLevels, branchesLoading: false })
          await get().computeForkPoints()
        } catch (err) {
          console.error('[Chat.store] Failed to load branches:', err)
          set({ branchesLoading: false })
        }
      },

      activateBranch: async (conversationId: string, branchId: string) => {
        await ApiClient.Branch.activate({ id: conversationId, branch_id: branchId })

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
          const forkLevel = groupKey.slice(separatorIdx + 2) as 'user' | 'assistant'

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
          } else if (activeBranchId && childBranchIds.includes(activeBranchId) && currentBranch) {
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
        // Clear text input first
        ;(get() as any).TextStore?.clearText()

        // Clear editing state — extensions react via their subscribe handlers
        set({
          editingMessage: null,
          pendingBranchFromMessageId: null,
          pendingBranchForkLevel: null,
        })

        // Reload messages to restore what was trimmed by startEditMessage
        const conversationId = get().conversation?.id
        if (conversationId) {
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

        // Restore file attachments from the user message so they are included
        // in the regenerated request. The MessageContentDataFileAttachment block
        // only carries file_id/filename/file_size/mime_type — remaining
        // FileEntity fields use defaults because sendMessage() fires immediately
        // and clearFiles() runs right after, so an async server fetch would
        // never complete in time to be useful.
        const fileContents = precedingUserMsg.contents.filter(
          c => c.content_type === 'file_attachment'
        )
        if (fileContents.length > 0) {
          // File store moved out of Stores.Chat into its own module
          // (modules/file/) — async-import to avoid a circular-dep
          // between chat and file.
          const { Stores } = await import('@/core/stores')
          const fileStore = Stores.File
          if (fileStore) {
            const stubs = fileContents.map(c => {
              const data = c.content as any
              return {
                id: data.file_id,
                filename: data.filename,
                file_size: data.file_size,
                mime_type: data.mime_type ?? undefined,
                has_thumbnail: false,
                preview_page_count: 0,
                created_at: '',
                updated_at: '',
                user_id: '',
                created_by: '',
                processing_metadata: null,
                text_page_count: 0,
              }
            })
            fileStore.restoreFilesFromEdit(stubs)
          }
        }

        if (!userText) return

        // Pre-fill text input with the original user message text
        ;(get() as any).TextStore?.setText(userText)

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

      sendMessage: async () => {
        let { conversation } = get()

        const beforeResult = await chatExtensionRegistry.beforeSendMessage()

        if (beforeResult.cancel) {
          console.log('[Chat.store] Message send cancelled by extension')
          throw new Error(beforeResult.errorMessage || 'Message send was cancelled')
        }

        // Collect all request fields from extensions
        const allRequestFields = await chatExtensionRegistry.composeRequestFields()

        // Inject branching fields directly (moved from branching extension)
        const pendingBranchFromMessageId = get().pendingBranchFromMessageId
        if (pendingBranchFromMessageId) {
          allRequestFields.create_branch_from_message_id = pendingBranchFromMessageId
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
          const afterHook = await chatExtensionRegistry.afterCreateConversation(
            conversation,
          )
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

        const streamConversationId = conversation.id

        set({ sending: true, isStreaming: true, error: null })

        const userContents = await chatExtensionRegistry.provideUserContent(
          allRequestFields.content as string || '',
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
          await ApiClient.Message.sendStream(
            {
              id: conversation.id,
              branch_id: conversation.active_branch_id || '',
              ...allRequestFields,
            } as any,
            {
              SSE: {
                __init: async (data: { abortController: AbortController }) => {
                  set({ sending: false, streamingAbortController: data.abortController })

                  await chatExtensionRegistry.onMessageSent()

                  // Clear pending branch state after message is sent
                  get().clearPendingBranch()
                },
                started: async data => {
                  await chatExtensionRegistry.onStreamStart()

                  // Detect branch change (moved from branching extension handleSSEEvent)
                  const currentBranchId = get().conversation?.active_branch_id
                  if (data.branch_id && data.branch_id !== currentBranchId) {
                    set(state => ({
                      conversation: state.conversation
                        ? { ...state.conversation, active_branch_id: data.branch_id }
                        : null,
                      branchChangedDuringStream: true,
                    }))
                    // Capture fork level before clearPendingBranch() clears it
                    get().captureBranchForkLevel(data.branch_id!)

                    // Reload branches for the navigator
                    const conversation = get().conversation
                    if (conversation) {
                      await get().loadBranches(conversation.id)
                    }
                  }

                  // Route through extensions
                  const sseEvent: SSEEvent = {
                    event_type: 'started',
                    data,
                  }
                  const handled = await chatExtensionRegistry.handleSSEEvent(sseEvent)

                  if (!handled) {
                    const state = get()
                    if (data.user_message_id && state.tempUserMessageId) {
                      const tempMessage = state.messages.get(state.tempUserMessageId)
                      if (tempMessage) {
                        set(state => {
                          const newMessages = new Map(state.messages)
                          newMessages.delete(state.tempUserMessageId!)

                          const updatedMessage = {
                            ...tempMessage,
                            id: data.user_message_id!,
                            contents: tempMessage.contents.map(content => ({
                              ...content,
                              message_id: data.user_message_id!,
                            })),
                          }

                          newMessages.set(data.user_message_id!, updatedMessage)

                          return {
                            messages: newMessages,
                            tempUserMessageId: null,
                          }
                        })
                      }
                    }
                    console.log('Chat stream started:', {
                      user_message_id: data.user_message_id,
                      conversation_id: data.conversation_id,
                      branch_id: data.branch_id,
                    })
                  }
                },
                content: async data => {
                  const sseEvent: SSEEvent = {
                    event_type: 'content',
                    data,
                  }
                  const handled = await chatExtensionRegistry.handleSSEEvent(sseEvent)

                  if (!handled) {
                    if (get().conversation?.id !== streamConversationId) return

                    const state = get()

                    if (data.content && Array.isArray(data.content)) {
                      // Any content-block delta (text, thinking, tool_use)
                      // is the LLM emitting *something* on the assistant
                      // turn. Ensure a placeholder streaming message is in
                      // the messages map BEFORE branching on block type, so
                      // the DOM bubble (`[data-role="assistant"]`) renders
                      // immediately even when the first events are
                      // tool_use_delta (which the MCP extension turns into
                      // a real tool_use content block via mcpToolStart only
                      // after the tool starts executing). Without this, the
                      // bubble appears late on tool-first flows and tests /
                      // users see a blank wait.
                      if (!state.streamingMessage && data.content.length > 0) {
                        const placeholderId =
                          data.message_id || `streaming-${Date.now()}`
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
                          return {
                            streamingMessage: placeholder,
                            messages: newMessages,
                          }
                        })
                      }

                      for (const block of data.content) {
                        if (block.type === 'text_delta') {
                          // First text_delta: hydrate the placeholder (or
                          // create a new streaming message if none yet —
                          // covers the original text-only path).
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

                            const initialContent = await chatExtensionRegistry.provideStreamingContent(
                              'text',
                              block.delta,
                            )

                            if (initialContent) {
                              const baseMessage =
                                currentState.streamingMessage ?? {
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
                              if (!currentState.streamingMessage) {
                                return {}
                              }

                              // Keep `streamingMessage.id` STABLE as the
                              // original placeholder throughout the stream
                              // (audit 04 HIGH-2 — was changing mid-stream
                              // when the backend's real DB id arrived,
                              // forcing React to remount the ChatMessage
                              // component on every key change and tearing
                              // down any local state inside it). The real
                              // DB id is still propagated into each
                              // content's `message_id` field below; on
                              // stream complete, `loadMessages()` reloads
                              // the authoritative messages keyed by the
                              // real id, so no data is lost.
                              const stableId = currentState.streamingMessage.id
                              const dbId = incomingMessageId || stableId

                              const textContentIndex = currentState.streamingMessage.contents.findIndex(
                                c => c.content_type === 'text' || (c.content as any)?.type === 'text'
                              )

                              let updatedContents: MessageContent[]

                              if (textContentIndex >= 0) {
                                const currentContent = currentState.streamingMessage.contents[textContentIndex]
                                const currentText = (currentContent.content as any)?.text || ''
                                const updatedContent: MessageContent = {
                                  ...currentContent,
                                  content: {
                                    ...currentContent.content,
                                    text: currentText + delta,
                                  } as any,
                                }

                                updatedContents = [...currentState.streamingMessage.contents]
                                updatedContents[textContentIndex] = updatedContent
                              } else {
                                const now = new Date().toISOString()
                                const newContent: MessageContent = {
                                  id: `${stableId}-content-${currentState.streamingMessage.contents.length}`,
                                  message_id: dbId,
                                  content_type: 'text',
                                  content: { type: 'text', text: delta } as any,
                                  sequence_order: currentState.streamingMessage.contents.length,
                                  created_at: now,
                                  updated_at: now,
                                }
                                updatedContents = [...currentState.streamingMessage.contents, newContent]
                              }

                              const updatedMessage: MessageWithContent = {
                                ...currentState.streamingMessage,
                                // id unchanged — see comment above
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
                  }
                },
                complete: async _data => {
                  const sseEvent: SSEEvent = {
                    event_type: 'complete',
                    data: _data,
                  }
                  const handled = await chatExtensionRegistry.handleSSEEvent(sseEvent)

                  if (!handled) {
                    const { streamingMessage } = get()
                    const isOnOriginalConversation = get().conversation?.id === streamConversationId

                    // Remove the streaming message from the messages map so it doesn't
                    // briefly coexist with DB messages during the async loadMessages call
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
                        messages: newMessages,
                      }
                    })

                    if (isOnOriginalConversation) {
                      if (streamingMessage) {
                        await chatExtensionRegistry.afterStreamComplete(streamingMessage)
                      }

                      // Always reload messages after stream completes so the UI
                      // reflects authoritative server state (including file_attachment blocks)
                      set({ branchChangedDuringStream: false })
                      const conversation = get().conversation
                      if (conversation) {
                        await get().loadMessages(conversation.id)

                        // Notify ChatHistory of the updated message count
                        const { Stores } = await import('@/core/stores')
                        await Stores.EventBus.emit({
                          type: 'conversation.messageCountChanged',
                          data: {
                            conversationId: conversation.id,
                            messageCount: get().messages.size,
                          },
                        })
                      }

                      // Always recompute fork points so the navigator is up to date
                      await get().computeForkPoints()
                    } else {
                      // Invalidate A's stale snapshot so messages reload fresh when user returns
                      get().clearConversationCache(streamConversationId)
                    }
                  }
                },
                error: async data => {
                  const streamError = new Error(data.message || 'Stream error')
                  await chatExtensionRegistry.onStreamError(streamError)

                  const sseEvent: SSEEvent = {
                    event_type: 'error',
                    data,
                  }
                  await chatExtensionRegistry.handleSSEEvent(sseEvent)

                  const isOnOriginalConversation = get().conversation?.id === streamConversationId

                  if (!isOnOriginalConversation) {
                    set({ isStreaming: false, sending: false, streamingMessage: null, streamingAbortController: null })
                    get().clearConversationCache(streamConversationId)
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
                        error: data.message || 'Stream error',
                        isStreaming: false,
                        sending: false,
                        streamingMessage: null,
                        streamingAbortController: null,
                      }
                    })
                  } else {
                    set({
                      error: data.message || 'Stream error',
                      isStreaming: false,
                      sending: false,
                      streamingMessage: null,
                      streamingAbortController: null,
                    })
                  }
                },
                default: async (event, data) => {
                  const sseEvent: GenericSSEEvent = {
                    event_type: event,
                    data,
                  }
                  const handled = await chatExtensionRegistry.handleSSEEvent(sseEvent)

                  if (!handled) {
                    console.log('Unknown chat SSE event:', event, data)
                  }
                },
              },
            },
          )
        } catch (error: any) {
          const isAborted = error instanceof Error && error.name === 'AbortError'

          if (!isAborted) {
            await chatExtensionRegistry.onStreamError(
              error instanceof Error ? error : new Error(error.message || 'Failed to send message')
            )
          }

          const state = get()
          const baseUpdate = {
            error: isAborted ? null : (error.message || 'Failed to send message'),
            sending: false,
            isStreaming: false,
            streamingMessage: null,
            streamingAbortController: null,
          }

          if (state.tempUserMessageId) {
            set(state => {
              const newMessages = new Map(state.messages)
              newMessages.delete(state.tempUserMessageId!)
              return { messages: newMessages, tempUserMessageId: null, ...baseUpdate }
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

      updateConversation: async (updates: {
        title?: string
      }) => {
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
        get().streamingAbortController?.abort()
      },

      displayInRightPanel: <T extends PanelType>(entry: RightPanelTab<T>) => {
        set(state => {
          const exists = state.rightPanel.tabs.some(t => t.id === entry.id)
          if (exists) {
            return { rightPanel: { ...state.rightPanel, activeId: entry.id, mobileDrawerOpen: true } }
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
          savePanelSnapshotForConversation(conversation.id, rightPanel.tabs, rightPanel.activeId)
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
          const mobileDrawerOpen = tabs.length > 0 ? state.rightPanel.mobileDrawerOpen : false
          return { rightPanel: { ...state.rightPanel, tabs, activeId, mobileDrawerOpen } }
        })
        const { rightPanel, conversation } = get()
        if (conversation) {
          savePanelSnapshotForConversation(conversation.id, rightPanel.tabs, rightPanel.activeId)
        }
      },

      closeAllRightPanelTabs: () => {
        set(state => ({ rightPanel: { ...state.rightPanel, tabs: [], activeId: null, mobileDrawerOpen: false } }))
        const { conversation } = get()
        if (conversation) {
          savePanelSnapshotForConversation(conversation.id, [], null)
        }
      },

      closeMobileDrawer: () => {
        set(state => ({ rightPanel: { ...state.rightPanel, mobileDrawerOpen: false } }))
      },

      setRightPanelWidth: (width: number) => {
        set(state => ({ rightPanel: { ...state.rightPanel, panelWidth: width } }))
      },

      reset: async () => {
        const { conversation } = get()
        if (conversation) {
          get().saveConversationState(conversation.id)
          get().scheduleCacheClear(conversation.id)

          // Save outgoing conversation's panel tabs to localStorage before clearing
          const { rightPanel } = get()
          savePanelSnapshotForConversation(conversation.id, rightPanel.tabs, rightPanel.activeId)

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
          streamingMessage: null,
          tempUserMessageId: null,
          branches: [],
          branchesLoading: false,
          pendingBranchFromMessageId: null,
          pendingBranchForkLevel: null,
          branchForkLevels: new Map(),
          branchChangedDuringStream: false,
          forkPoints: new Map(),
          editingMessage: null,
          rightPanel: { ...state.rightPanel, tabs: [], activeId: null, mobileDrawerOpen: false },
        }))
      },

      // ── Lifecycle methods ──────────────────────────────────────────────────

      __init__: {
        __store__: () => {
          console.log('[Chat.store] Initialized')
        },
      },

      __destroy__: () => {
        console.log('[Chat.store] Destroying - cleaning up resources')

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

        console.log('[Chat.store] Destroyed successfully')
      },
    })),
)
