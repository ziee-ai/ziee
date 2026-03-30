import { create } from 'zustand'
import { subscribeWithSelector } from 'zustand/middleware'
import { ApiClient } from '@/api-client'
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

  createConversation: (title?: string) => Promise<Conversation>
  loadConversation: (id: string) => Promise<void>
  loadMessages: (id: string) => Promise<void>
  sendMessage: () => Promise<void>
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

      createConversation: async (title?: string) => {
        set({ loading: true, error: null })
        try {
          const conversation = await ApiClient.Conversation.create({
            title: title,
          })
          set({ conversation, loading: false })

          const { Stores } = await import('@/core/stores')
          await Stores.EventBus.emit({
            type: 'conversation.created',
            data: { conversation },
          })

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

          await chatExtensionRegistry.cleanup()
          set({ isStreaming: false, sending: false, streamingMessage: null, tempUserMessageId: null })
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
          return
        }

        console.log(`[Chat.store] Cache miss for conversation: ${id}`)
        set({ loading: true, loadingConversationId: id, error: null })
        try {
          const conversation = await ApiClient.Conversation.get({ id })
          set({ conversation, loading: false, loadingConversationId: null })

          await get().loadMessages(id)
          await get().loadBranches(id)

          await chatExtensionRegistry.initialize()
          await chatExtensionRegistry.onConversationLoad(conversation)
        } catch (error: any) {
          set({
            error: error.message || 'Failed to load conversation',
            loading: false,
            loadingConversationId: null,
          })
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
          conversation = await get().createConversation()
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
                __init: async _data => {
                  console.log('Chat SSE initialized with abortController')
                  set({ sending: false })

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
                      for (const block of data.content) {
                        if (block.type === 'text_delta') {
                          if (!state.streamingMessage) {
                            const messageId =
                              data.message_id || `streaming-${Date.now()}`

                            const initialContent = await chatExtensionRegistry.provideStreamingContent(
                              'text',
                              block.delta,
                            )

                            if (initialContent) {
                              const newMessage: MessageWithContent = {
                                id: messageId,
                                role: 'assistant',
                                contents: [
                                  {
                                    ...initialContent,
                                    id: `${messageId}-content-0`,
                                    message_id: messageId,
                                  },
                                ],
                                originated_from_id: '',
                                edit_count: 0,
                                created_at: new Date().toISOString(),
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

                              const messageId = incomingMessageId || currentState.streamingMessage.id
                              const idChanged = messageId !== currentState.streamingMessage.id

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
                                  id: `${messageId}-content-${currentState.streamingMessage.contents.length}`,
                                  message_id: messageId,
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
                                id: messageId,
                                contents: updatedContents.map(c => ({
                                  ...c,
                                  message_id: messageId,
                                })),
                              }

                              const newMessages = new Map(currentState.messages)
                              if (idChanged) {
                                newMessages.delete(currentState.streamingMessage.id)
                              }
                              newMessages.set(updatedMessage.id, updatedMessage)

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

                    set({
                      isStreaming: false,
                      sending: false,
                      streamingMessage: null,
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
                    set({ isStreaming: false, sending: false, streamingMessage: null })
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
                      }
                    })
                  } else {
                    set({
                      error: data.message || 'Stream error',
                      isStreaming: false,
                      sending: false,
                      streamingMessage: null,
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
          await chatExtensionRegistry.onStreamError(
            error instanceof Error ? error : new Error(error.message || 'Failed to send message')
          )

          const state = get()

          if (state.tempUserMessageId) {
            set(state => {
              const newMessages = new Map(state.messages)
              newMessages.delete(state.tempUserMessageId!)
              return {
                messages: newMessages,
                tempUserMessageId: null,
                error: error.message || 'Failed to send message',
                sending: false,
                isStreaming: false,
                streamingMessage: null,
              }
            })
          } else {
            set({
              error: error.message || 'Failed to send message',
              sending: false,
              isStreaming: false,
              streamingMessage: null,
            })
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

      reset: async () => {
        const { conversation } = get()
        if (conversation) {
          get().saveConversationState(conversation.id)
          get().scheduleCacheClear(conversation.id)
          await chatExtensionRegistry.cleanup()
        }

        set({
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
        })
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
