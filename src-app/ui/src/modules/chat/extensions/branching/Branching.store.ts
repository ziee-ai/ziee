import { createExtensionStore } from '@/modules/chat/core/extensions'
import { ApiClient } from '@/api-client'
import type { Branch } from '@/api-client/types'
import { computeParentAnchor, computeChildAnchor } from './branchAnchor.utils'

interface BranchingStore {
  /** All branches for the current conversation */
  branches: Branch[]
  branchesLoading: boolean

  /**
   * Message ID to create a new branch from on the next sendMessage call.
   * Used by Regenerate (set in MessageActions) and Edit (set in confirmEdit).
   * Cleared in onMessageSent after the request is dispatched.
   */
  pendingBranchFromMessageId: string | null

  /**
   * The fork level for the next branch to be created.
   * - 'assistant': regenerate flow — fork is at a user message but the navigator
   *   should anchor at the assistant bubble (because both branches share the user message).
   * - 'user': edit flow — fork is at a user message and the navigator anchors there.
   * - null: no pending branch.
   *
   * Set before sendMessage and captured into branchForkLevels when the SSE
   * 'started' event confirms the new branch_id.
   */
  pendingBranchForkLevel: 'user' | 'assistant' | null

  /**
   * Per-branch fork level map. Persists the fork level set at branch creation
   * time so computeForkPoints can determine the correct anchor even after the
   * pendingBranchForkLevel has been cleared.
   *
   * Populated by captureBranchForkLevel() when handleSSEEvent sees a new branch_id.
   * In-memory only — defaults to 'user' on page reload (minor limitation).
   */
  branchForkLevels: Map<string, 'user' | 'assistant'>

  /**
   * Inline editing state — which message bubble is currently open for editing.
   * null means no message is being edited.
   */
  editingMessageId: string | null

  /** Current text inside the inline editor textarea */
  editingText: string

  /**
   * Set to true when the SSE 'started' event reveals that a new branch was
   * created for this stream (create_branch_from_message_id was used).
   * Cleared in afterStreamComplete after reloading messages.
   */
  branchChangedDuringStream: boolean

  /**
   * Per-message fork points.
   * Maps a message ID (visible in the current view) to an ordered list of
   * branch IDs that diverge at that message. The list is sorted by branch
   * created_at so the oldest branch is always index 0.
   *
   * Used by MessageBranchNavigator to render < X/N > at the right bubble.
   */
  forkPoints: Map<string, string[]>

  // ── Getters (functions so the proxy returns them without calling React hooks) ──

  /**
   * Safe getter for pendingBranchFromMessageId.
   * Must be called as a function because accessing state values through the
   * store proxy outside a React component would invoke React hooks and throw.
   * Following the same pattern as TextStore.getText().
   */
  getPendingBranchFromMessageId: () => string | null

  /** Safe getter for pendingBranchForkLevel — returns value without calling hooks */
  getPendingBranchForkLevel: () => 'user' | 'assistant' | null

  /**
   * Set the fork level for the next branch to be created.
   * Call before setPendingBranchFromMessage when the fork level is known
   * (e.g., 'assistant' for regenerate).
   */
  setPendingBranchForkLevel: (level: 'user' | 'assistant' | null) => void

  /**
   * Capture the fork level into branchForkLevels for a confirmed branch_id.
   * Called by handleSSEEvent when the 'started' event confirms a new branch was created.
   * Reads pendingBranchForkLevel and stores it under branchId.
   */
  captureBranchForkLevel: (branchId: string) => void

  /** Safe getter for branchChangedDuringStream — returns value without calling hooks */
  getBranchChangedDuringStream: () => boolean

  /** Set or clear the branchChangedDuringStream flag */
  setBranchChangedDuringStream: (value: boolean) => void

  // ── Actions ──

  loadBranches: (conversationId: string) => Promise<void>
  setPendingBranchFromMessage: (messageId: string | null) => void
  activateBranch: (conversationId: string, branchId: string) => Promise<void>

  /**
   * Recompute forkPoints from the current branches + Chat store messages.
   * Called after loadBranches, activateBranch, and afterStreamComplete so
   * the per-message navigator is always up to date.
   */
  computeForkPoints: () => Promise<void>

  /**
   * Remove the fork message and all messages after it from the Chat store so
   * the UI immediately shows the correct branch base before sendMessage runs.
   * Called by confirmEdit and handleRegenerate right before sendMessage.
   */
  trimMessagesToForkPoint: (forkMessageId: string) => Promise<void>

  /** Open the inline editor for a user message, pre-populated with its current text */
  startEditing: (messageId: string, originalText: string) => void

  /** Update the text as the user types in the inline editor */
  updateEditingText: (text: string) => void

  /** Close the inline editor without saving */
  cancelEditing: () => void

  /**
   * Confirm the inline edit:
   * 1. Sets pendingBranchFromMessageId so the next send creates a new branch
   *    from the original message position (excluding it).
   * 2. Pre-fills TextStore with the edited text.
   * 3. Triggers sendMessage() so the AI generates a response on the new branch.
   */
  confirmEdit: () => Promise<void>
}

export const createBranchingStore = () =>
  createExtensionStore<BranchingStore>((set, get) => ({
    branches: [],
    branchesLoading: false,
    pendingBranchFromMessageId: null,
    pendingBranchForkLevel: null,
    branchForkLevels: new Map(),
    editingMessageId: null,
    editingText: '',
    branchChangedDuringStream: false,
    forkPoints: new Map(),

    getPendingBranchFromMessageId: () => get().pendingBranchFromMessageId,

    getPendingBranchForkLevel: () => get().pendingBranchForkLevel,

    setPendingBranchForkLevel: (level: 'user' | 'assistant' | null) => {
      set(state => {
        state.pendingBranchForkLevel = level
      })
    },

    captureBranchForkLevel: (branchId: string) => {
      const level = get().pendingBranchForkLevel
      set(state => {
        state.branchForkLevels.set(branchId, level ?? 'user')
        state.pendingBranchForkLevel = null
      })
    },

    getBranchChangedDuringStream: () => get().branchChangedDuringStream,

    setBranchChangedDuringStream: (value: boolean) => {
      set(state => {
        state.branchChangedDuringStream = value
      })
    },

    loadBranches: async (conversationId: string) => {
      set(state => {
        state.branchesLoading = true
      })
      try {
        const branches = await ApiClient.Branch.list({ id: conversationId })
        set(state => {
          state.branches = branches
          state.branchesLoading = false
        })
        await get().computeForkPoints()
      } catch (err) {
        console.error('[BranchingStore] Failed to load branches:', err)
        set(state => {
          state.branchesLoading = false
        })
      }
    },

    setPendingBranchFromMessage: (messageId: string | null) => {
      set(state => {
        state.pendingBranchFromMessageId = messageId
      })
    },

    trimMessagesToForkPoint: async (forkMessageId: string) => {
      const { useChatStore } = await import(
        '@/modules/chat/core/stores/Chat.store'
      )
      useChatStore.setState(state => {
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

    activateBranch: async (conversationId: string, branchId: string) => {
      await ApiClient.Branch.activate({ id: conversationId, branch_id: branchId })

      const { useChatStore } = await import(
        '@/modules/chat/core/stores/Chat.store'
      )
      useChatStore.setState(state => ({
        conversation: state.conversation
          ? { ...state.conversation, active_branch_id: branchId }
          : null,
      }))

      await useChatStore.getState().loadMessages(conversationId)

      const { branches } = get()
      if (!branches.find(b => b.id === branchId)) {
        await get().loadBranches(conversationId)
      } else {
        await get().computeForkPoints()
      }
    },

    computeForkPoints: async () => {
      const { useChatStore } = await import(
        '@/modules/chat/core/stores/Chat.store'
      )
      const chatState = useChatStore.getState()
      const { branches } = get()

      const conversation = chatState.conversation
      if (!conversation || branches.length <= 1) {
        set(state => {
          state.forkPoints = new Map()
        })
        return
      }

      const activeBranchId = conversation.active_branch_id
      const messages = [...chatState.messages.values()].sort(
        (a, b) =>
          new Date(a.created_at).getTime() - new Date(b.created_at).getTime(),
      )
      const messageIds = new Set(messages.map(m => m.id))

      const forkPoints = new Map<string, string[]>()

      // Group child branches by a composite key: `${created_from_message_id}__${forkLevel}`.
      //
      // Using both dimensions as the key is critical: a user message can be the fork
      // origin for two independent sets of branches — one created by Regenerate
      // ('assistant' level) and one created by Edit ('user' level). Grouping by message
      // ID alone would merge them into a single navigator showing 1/3, 1/4, etc.
      // Splitting by level ensures each produces its own independent navigator.
      const forkGroups = new Map<string, string[]>()
      for (const branch of branches) {
        if (branch.created_from_message_id) {
          const forkLevel = get().branchForkLevels.get(branch.id) ?? 'user'
          const key = `${branch.created_from_message_id}__${forkLevel}`
          if (!forkGroups.has(key)) {
            forkGroups.set(key, [])
          }
          forkGroups.get(key)!.push(branch.id)
        }
      }

      const currentBranch = branches.find(b => b.id === activeBranchId)

      for (const [groupKey, childBranchIds] of forkGroups) {
        // Parse the composite key back into fork message ID and fork level.
        // UUIDs never contain '__' so splitting on the last '__' is unambiguous.
        const separatorIdx = groupKey.lastIndexOf('__')
        const forkMsgId = groupKey.slice(0, separatorIdx)
        const forkLevel = groupKey.slice(separatorIdx + 2) as 'user' | 'assistant'

        // Find the parent branch (the branch that owns forkMsgId in its history)
        const firstChild = branches.find(b => b.id === childBranchIds[0])
        const parentBranchId = firstChild?.parent_branch_id

        // Fork group: parent branch + all children diverging at this (forkMsgId, forkLevel)
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

        // Determine which message in the current view anchors this navigator
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
            get().branchForkLevels,
          )
        }

        if (anchorMessageId) {
          forkPoints.set(anchorMessageId, sortedGroupIds)
        }
      }

      set(state => {
        state.forkPoints = forkPoints
      })
    },

    startEditing: (messageId: string, originalText: string) => {
      set(state => {
        state.editingMessageId = messageId
        state.editingText = originalText
      })
    },

    updateEditingText: (text: string) => {
      set(state => {
        state.editingText = text
      })
    },

    cancelEditing: () => {
      set(state => {
        state.editingMessageId = null
        state.editingText = ''
      })
    },

    confirmEdit: async () => {
      const { editingMessageId, editingText } = get()
      if (!editingMessageId || !editingText.trim()) return

      // Commit the editing state before calling sendMessage
      set(state => {
        state.pendingBranchFromMessageId = editingMessageId
        state.editingMessageId = null
        state.editingText = ''
      })

      // Pre-fill TextStore with the edited content so sendMessage picks it up
      // (setText is a function — safe to call through the proxy outside a component)
      const { Stores } = await import('@/core/stores')
      Stores.Chat.__state.TextStore.setText(editingText)

      // Trim stale messages immediately so the UI shows the correct branch
      // base before sendMessage runs (avoids layout shift after stream ends)
      await get().trimMessagesToForkPoint(editingMessageId)

      // sendMessage is a function on the Chat store — safe to call outside a component
      await Stores.Chat.sendMessage()
    },
  }))

/**
 * Augment ChatExtensionStores with BranchingStore
 */
declare module '../../types' {
  interface ChatExtensionStores {
    BranchingStore: ReturnType<typeof createBranchingStore>
  }
}
