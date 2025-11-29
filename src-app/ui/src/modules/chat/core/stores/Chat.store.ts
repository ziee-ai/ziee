import { create } from 'zustand'
import { subscribeWithSelector } from 'zustand/middleware'
import { ApiClient } from '@/api-client'
import type {
  Conversation,
  MessageWithContent,
  MessageContentData,
} from '@/api-client/types'
import { chatExtensionRegistry } from '../../extensions'
import type { SSEEvent, GenericSSEEvent } from '../extensions/types'

/**
 * Snapshot of conversation state for caching
 */
interface ChatStateSnapshot {
  conversation: Conversation | null
  messages: Map<string, MessageWithContent>
  streamingMessage: MessageWithContent | null
  tempUserMessageId: string | null
}

interface ChatState {
  // Data
  conversation: Conversation | null
  messages: Map<string, MessageWithContent>

  // Loading states
  loading: boolean
  sending: boolean
  isStreaming: boolean
  error: string | null

  // Streaming message assembly
  streamingMessage: MessageWithContent | null
  tempUserMessageId: string | null

  // Conversation state cache (whole-store snapshots)
  conversationStateCache: Map<string, ChatStateSnapshot>
  cacheClearTimers: Map<string, NodeJS.Timeout>

  // Conversation state management
  saveConversationState: (conversationId: string) => void
  loadConversationState: (conversationId: string) => boolean
  scheduleCacheClear: (conversationId: string, delayMs?: number) => void
  cancelCacheClear: (conversationId: string) => void
  clearConversationCache: (conversationId: string) => void

  // Actions
  createConversation: (modelId: string, title?: string) => Promise<Conversation>
  loadConversation: (id: string) => Promise<void>
  loadMessages: (id: string) => Promise<void>
  sendMessage: (content: string, modelId: string) => Promise<void>
  updateConversation: (updates: { title?: string }) => Promise<void>
  clearError: () => void
  reset: () => void

  // Lifecycle methods
  __init__: {
    __store__?: () => void
  }
  __destroy__?: () => void
}

export const useChatStore = create<ChatState>()(
  subscribeWithSelector((set, get) => ({
      // Initial state
      conversation: null,
      messages: new Map<string, MessageWithContent>(),
      loading: false,
      sending: false,
      isStreaming: false,
      error: null,
      streamingMessage: null,
      tempUserMessageId: null,

      // Conversation state cache (whole-store snapshots)
      conversationStateCache: new Map<string, ChatStateSnapshot>(),
      cacheClearTimers: new Map<string, NodeJS.Timeout>(),

      /**
       * Save current conversation state to cache
       * Creates a snapshot of the entire state for later restoration
       */
      saveConversationState: (conversationId: string) => {
        const state = get()
        const snapshot: ChatStateSnapshot = {
          conversation: state.conversation,
          messages: new Map(state.messages),
          streamingMessage: state.streamingMessage,
          tempUserMessageId: state.tempUserMessageId,
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

      /**
       * Load conversation state from cache
       * Restores the entire state from a previous snapshot
       * @returns true if cache hit, false if cache miss
       */
      loadConversationState: (conversationId: string): boolean => {
        const state = get()
        const snapshot = state.conversationStateCache.get(conversationId)
        if (!snapshot) {
          console.log(
            `[Chat.store] Cache miss for conversation: ${conversationId}`,
          )
          return false // Cache miss
        }

        set({
          conversation: snapshot.conversation,
          messages: new Map(snapshot.messages),
          streamingMessage: snapshot.streamingMessage,
          tempUserMessageId: snapshot.tempUserMessageId,
        })
        console.log(
          `[Chat.store] Cache hit - restored conversation state for: ${conversationId}`,
        )
        return true // Cache hit
      },

      /**
       * Schedule cache clear for a conversation
       * Clears cache after delay (default 5 minutes)
       * Can be cancelled by calling cancelCacheClear
       */
      scheduleCacheClear: (
        conversationId: string,
        delayMs: number = 5 * 60 * 1000,
      ) => {
        // Cancel existing timer if any
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

      /**
       * Cancel scheduled cache clear for a conversation
       * Call this when conversation is remounted before timer expires
       */
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

      /**
       * Clear cached state for a conversation
       * Removes the snapshot and any pending timers
       */
      clearConversationCache: (conversationId: string) => {
        get().cancelCacheClear(conversationId) // Cancel any pending timer
        set(state => {
          const newCache = new Map(state.conversationStateCache)
          newCache.delete(conversationId)
          return { conversationStateCache: newCache }
        })
        console.log(
          `[Chat.store] Cleared cache for conversation: ${conversationId}`,
        )
      },

      // Create new conversation
      createConversation: async (modelId: string, title?: string) => {
        set({ loading: true, error: null })
        try {
          const conversation = await ApiClient.Conversation.create({
            model_id: modelId,
            title: title,
          })
          set({ conversation, loading: false })

          // Emit conversation.created event
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

      // Load conversation by ID
      loadConversation: async (id: string) => {
        const currentConversation = get().conversation

        // If switching to a different conversation, save current state first
        if (currentConversation && currentConversation.id !== id) {
          console.log(
            `[Chat.store] Switching from ${currentConversation.id} to ${id} - saving current state`,
          )
          get().saveConversationState(currentConversation.id)
          get().scheduleCacheClear(currentConversation.id)

          // Cleanup extensions for current conversation
          await chatExtensionRegistry.cleanup()
        }

        // Cancel any pending cache clear timer for the new conversation
        get().cancelCacheClear(id)

        // Try to load from cache first
        const cacheHit = get().loadConversationState(id)
        if (cacheHit) {
          // Cache hit - state already restored, just initialize extensions
          console.log(`[Chat.store] Cache hit for conversation: ${id}`)
          await chatExtensionRegistry.initialize()
          return
        }

        // Cache miss - load from API
        console.log(`[Chat.store] Cache miss for conversation: ${id}`)
        set({ loading: true, error: null })
        try {
          const conversation = await ApiClient.Conversation.get({ id })
          set({ conversation, loading: false })

          // Load messages for this conversation
          await get().loadMessages(id)

          // Initialize extensions for this conversation
          await chatExtensionRegistry.initialize()
        } catch (error: any) {
          set({
            error: error.message || 'Failed to load conversation',
            loading: false,
          })
        }
      },

      // Load messages for conversation
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

      // Send message with SSE streaming
      sendMessage: async (content: string, modelId: string) => {
        const { conversation } = get()

        if (!conversation) {
          set({ error: 'No active conversation' })
          return
        }

        // Let extensions modify message before sending
        const beforeResult = await chatExtensionRegistry.beforeSendMessage(
          content,
        )

        // Check if any extension cancelled the send
        if (beforeResult.cancel) {
          console.log('[Chat.store] Message send cancelled by extension')
          throw new Error(beforeResult.errorMessage || 'Message send was cancelled')
        }

        // Use modified message if provided by extension
        const finalContent = beforeResult.message || content

        // Collect request fields from all extensions
        const composedFields = await chatExtensionRegistry.composeRequestFields()

        // Merge all extension fields (composeRequestFields + beforeSendMessage fields)
        const allRequestFields = {
          ...composedFields,
          ...beforeResult.requestFields, // beforeSendMessage fields take precedence
        }

        set({ sending: true, isStreaming: true, error: null })

        // Create optimistic user message (using potentially modified content)
        const tempUserMessage: MessageWithContent = {
          id: `temp-${Date.now()}`,
          role: 'user',
          contents: [
            {
              id: `temp-content-${Date.now()}`,
              message_id: `temp-${Date.now()}`,
              content_type: 'text',
              content: { type: 'text', text: finalContent },
              sequence_order: 0,
              created_at: new Date().toISOString(),
              updated_at: new Date().toISOString(),
            },
          ],
          originated_from_id: '',
          edit_count: 0,
          created_at: new Date().toISOString(),
        }

        // Add optimistic user message and track its temp ID
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
              content: finalContent,
              model_id: modelId,
              branch_id: conversation.active_branch_id || '',
              // Include all custom fields from extensions
              ...allRequestFields,
            },
            {
              SSE: {
                __init: async _data => {
                  console.log('Chat SSE initialized with abortController')
                  set({ sending: false })

                  // Call onMessageSent hook after message is successfully sent
                  await chatExtensionRegistry.onMessageSent()
                },
                started: async data => {
                  // Call onStreamStart hook when streaming starts
                  await chatExtensionRegistry.onStreamStart()

                  // Route through extensions first
                  const sseEvent: SSEEvent = {
                    event_type: 'started',
                    data,
                  }
                  const handled = await chatExtensionRegistry.handleSSEEvent(
                    sseEvent,
                  )

                  // Always handle locally (extensions don't need to handle this)
                  if (!handled) {
                    // Handle started event - update optimistic user message with real IDs
                    const state = get()
                    if (
                      data.user_message_id &&
                      data.user_content_id &&
                      state.tempUserMessageId
                    ) {
                      const tempMessage = state.messages.get(
                        state.tempUserMessageId,
                      )
                      if (tempMessage) {
                        set(state => {
                          const newMessages = new Map(state.messages)
                          newMessages.delete(state.tempUserMessageId!)

                          // Update message with real IDs
                          const updatedMessage = {
                            ...tempMessage,
                            id: data.user_message_id!,
                            contents: tempMessage.contents.map(content => ({
                              ...content,
                              id: data.user_content_id!,
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
                      user_content_id: data.user_content_id,
                      conversation_id: data.conversation_id,
                      branch_id: data.branch_id,
                    })
                  }
                },
                content: async data => {
                  // Route through extensions first
                  const sseEvent: SSEEvent = {
                    event_type: 'content',
                    data,
                  }
                  const handled = await chatExtensionRegistry.handleSSEEvent(
                    sseEvent,
                  )

                  // Handle locally if not handled by extension
                  if (!handled) {
                    const state = get()

                    // Handle content chunks
                    if (data.content && Array.isArray(data.content)) {
                      for (const block of data.content) {
                        if (block.type === 'text_delta') {
                          // Initialize or update streaming message
                          if (!state.streamingMessage) {
                            // Create new assistant message with real ID from backend
                            const messageId =
                              data.message_id || `streaming-${Date.now()}`
                            const newMessage: MessageWithContent = {
                              id: messageId,
                              role: 'assistant',
                              contents: [
                                {
                                  id: `${messageId}-content-0`,
                                  message_id: messageId,
                                  content_type: 'text',
                                  content: { type: 'text', text: block.delta || '' },
                                  sequence_order: 0,
                                  created_at: new Date().toISOString(),
                                  updated_at: new Date().toISOString(),
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
                          } else {
                            // Append to existing message
                            const updatedMessage = {
                              ...state.streamingMessage,
                              contents: state.streamingMessage.contents.map(
                                c => ({
                                  ...c,
                                  content: (c.content.type === 'text'
                                    ? {
                                        type: 'text',
                                        text:
                                          (c.content.text || '') +
                                          (block.delta || ''),
                                      }
                                    : c.content) as MessageContentData,
                                }),
                              ),
                            }

                            set(state => {
                              const newMessages = new Map(state.messages)
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
                  // Route through extensions first
                  const sseEvent: SSEEvent = {
                    event_type: 'complete',
                    data: _data,
                  }
                  const handled = await chatExtensionRegistry.handleSSEEvent(
                    sseEvent,
                  )

                  // Always handle locally
                  if (!handled) {
                    // Streaming complete - reload messages to get final versions
                    get().loadMessages(conversation.id)
                    set({
                      isStreaming: false,
                      sending: false,
                      streamingMessage: null,
                    })
                  }
                },
                error: async data => {
                  // Call onStreamError hook when streaming encounters an error
                  const streamError = new Error(data.message || 'Stream error')
                  await chatExtensionRegistry.onStreamError(streamError)

                  // Route through extensions first
                  const sseEvent: SSEEvent = {
                    event_type: 'error',
                    data,
                  }
                  await chatExtensionRegistry.handleSSEEvent(sseEvent)

                  // Always handle errors locally
                  set({
                    error: data.message || 'Stream error',
                    isStreaming: false,
                    sending: false,
                    streamingMessage: null,
                  })
                },
                default: async (event, data) => {
                  // Route unknown events through extensions as GenericSSEEvent
                  const sseEvent: GenericSSEEvent = {
                    event_type: event,
                    data,
                  }
                  const handled = await chatExtensionRegistry.handleSSEEvent(
                    sseEvent,
                  )

                  // Log if not handled by any extension
                  if (!handled) {
                    console.log('Unknown chat SSE event:', event, data)
                  }
                },
              },
            },
          )
        } catch (error: any) {
          // Call onStreamError hook when stream initialization fails
          await chatExtensionRegistry.onStreamError(
            error instanceof Error ? error : new Error(error.message || 'Failed to send message')
          )

          set({
            error: error.message || 'Failed to send message',
            sending: false,
            isStreaming: false,
            streamingMessage: null,
          })
        }
      },

      // Update conversation properties (e.g., title)
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

          // Emit titleUpdated event if title was updated
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
        // Save snapshot and cleanup for current conversation
        const { conversation } = get()
        if (conversation) {
          // Save entire conversation state snapshot
          get().saveConversationState(conversation.id)

          // Schedule cache clear after 5 minutes
          get().scheduleCacheClear(conversation.id)

          await chatExtensionRegistry.cleanup()
        }

        set({
          conversation: null,
          messages: new Map<string, MessageWithContent>(),
          loading: false,
          sending: false,
          isStreaming: false,
          error: null,
          streamingMessage: null,
          tempUserMessageId: null,
        })
      },

      // Lifecycle methods
      __init__: {
        __store__: () => {
          // No event listeners currently needed
          // Could be extended later for:
          // - Event bus subscriptions
          // - Initial conversation loading
          console.log('[Chat.store] Initialized')
        },
      },

      __destroy__: () => {
        console.log('[Chat.store] Destroying - cleaning up resources')

        const state = get()

        // Clear ALL cache timers to prevent memory leaks
        for (const [
          conversationId,
          timer,
        ] of state.cacheClearTimers.entries()) {
          clearTimeout(timer)
          console.log(
            `[Chat.store] Cleared pending timer for conversation: ${conversationId}`,
          )
        }

        // Save current conversation state if exists
        if (state.conversation) {
          get().saveConversationState(state.conversation.id)

          // Cleanup extensions for current conversation
          // Run cleanup synchronously (can't await in destroy)
          chatExtensionRegistry
            .cleanup()
            .catch(error =>
              console.error('[Chat.store] Extension cleanup failed:', error),
            )
        }

        // Clear all cached conversations
        state.conversationStateCache.clear()
        state.cacheClearTimers.clear()

        console.log('[Chat.store] Destroyed successfully')
      },
    })),
)
