import { create } from 'zustand'
import { subscribeWithSelector } from 'zustand/middleware'
import { ApiClient } from '@/api-client'
import type {
  Conversation,
  MessageContent,
  MessageWithContent,
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

  // Conversation state management
  saveConversationState: (conversationId: string) => void
  loadConversationState: (conversationId: string) => boolean
  scheduleCacheClear: (conversationId: string, delayMs?: number) => void
  cancelCacheClear: (conversationId: string) => void
  clearConversationCache: (conversationId: string) => void

  // Actions
  createConversation: (title?: string) => Promise<Conversation>
  loadConversation: (id: string) => Promise<void>
  loadMessages: (id: string) => Promise<void>
  sendMessage: () => Promise<void>
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
      loadingConversationId: null,
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
      createConversation: async (title?: string) => {
        set({ loading: true, error: null })
        try {
          const conversation = await ApiClient.Conversation.create({
            title: title,
            // NO model_id - backend auto-updates on first message
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
        const loadingId = get().loadingConversationId

        // If conversation is already loaded, do nothing
        if (currentConversation && currentConversation.id === id) {
          console.log(`[Chat.store] Conversation ${id} already loaded, skipping`)
          return
        }

        // If already loading this conversation, do nothing
        if (loadingId === id) {
          console.log(`[Chat.store] Conversation ${id} is already loading, skipping`)
          return
        }

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

          // Notify extensions that conversation loaded
          const { conversation } = get()
          if (conversation) {
            await chatExtensionRegistry.onConversationLoad(conversation)
          }
          return
        }

        // Cache miss - load from API
        console.log(`[Chat.store] Cache miss for conversation: ${id}`)
        set({ loading: true, loadingConversationId: id, error: null })
        try {
          const conversation = await ApiClient.Conversation.get({ id })
          set({ conversation, loading: false, loadingConversationId: null })

          // Load messages for this conversation
          await get().loadMessages(id)

          // Notify extensions that conversation loaded
          await chatExtensionRegistry.onConversationLoad(conversation)
        } catch (error: any) {
          set({
            error: error.message || 'Failed to load conversation',
            loading: false,
            loadingConversationId: null,
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
      sendMessage: async () => {
        let { conversation } = get()

        // Validate message BEFORE creating conversation
        // Text extension validates text is not empty, file extension checks uploads, etc.
        const beforeResult = await chatExtensionRegistry.beforeSendMessage()

        // Check if any extension cancelled the send
        if (beforeResult.cancel) {
          console.log('[Chat.store] Message send cancelled by extension')
          throw new Error(beforeResult.errorMessage || 'Message send was cancelled')
        }

        // Collect all request fields from extensions BEFORE creating conversation
        // (createConversation can trigger re-renders that reset form state)
        // Text extension provides 'content', MCP provides 'enable_mcp', 'mcp_config', etc.
        const allRequestFields = await chatExtensionRegistry.composeRequestFields()

        // Create conversation if needed (only after validation passes)
        if (!conversation) {
          conversation = await get().createConversation()

          // Initialize extensions with new conversation
          await chatExtensionRegistry.onConversationLoad(conversation)
        }

        set({ sending: true, isStreaming: true, error: null })

        // Create optimistic user message using extension hooks
        // Extensions provide content (text, file attachments, etc.)
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
              branch_id: conversation.active_branch_id || '',
              // All fields from extensions (content, model_id, enable_mcp, etc.)
              ...allRequestFields,
            } as any,
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
                    // Handle started event - update optimistic user message with real message ID
                    const state = get()
                    if (data.user_message_id && state.tempUserMessageId) {
                      const tempMessage = state.messages.get(
                        state.tempUserMessageId,
                      )
                      if (tempMessage) {
                        set(state => {
                          const newMessages = new Map(state.messages)
                          newMessages.delete(state.tempUserMessageId!)

                          // Update message with real message ID
                          // Content IDs keep their frontend-generated UUIDs
                          const updatedMessage = {
                            ...tempMessage,
                            id: data.user_message_id!,
                            contents: tempMessage.contents.map(content => ({
                              ...content,
                              // id stays as temp UUID (don't update)
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
                          // Initialize or update streaming message using extensions
                          if (!state.streamingMessage) {
                            // Create new assistant message using extension hooks
                            const messageId =
                              data.message_id || `streaming-${Date.now()}`

                            // Ask extensions to provide initial content
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
                            // Append delta to streaming message
                            // IMPORTANT: Do all state mutations inside set() callback to avoid race conditions
                            // Multiple SSE events can arrive quickly, so we must use fresh state
                            const delta = block.delta || ''
                            const incomingMessageId = data.message_id

                            set(currentState => {
                              if (!currentState.streamingMessage) {
                                // Another handler might have cleared it
                                return {}
                              }

                              const messageId = incomingMessageId || currentState.streamingMessage.id
                              const idChanged = messageId !== currentState.streamingMessage.id

                              // Find existing text content block to append to
                              const textContentIndex = currentState.streamingMessage.contents.findIndex(
                                c => c.content_type === 'text' || (c.content as any)?.type === 'text'
                              )

                              let updatedContents: MessageContent[]

                              if (textContentIndex >= 0) {
                                // Append delta to existing text content
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
                                // No text content exists, create new one
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

                              // Update message ID if it changed (preserves link to backend message)
                              const updatedMessage: MessageWithContent = {
                                ...currentState.streamingMessage,
                                id: messageId,
                                contents: updatedContents.map(c => ({
                                  ...c,
                                  message_id: messageId,
                                })),
                              }

                              const newMessages = new Map(currentState.messages)
                              // Remove old ID entry if ID changed
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
                    // Streaming complete - messages are already in state from SSE events
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
                  const state = get()

                  // Remove optimistic user message if it still has temp ID
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
                    // Message already has real ID - it was successfully created, keep it
                    set({
                      error: data.message || 'Stream error',
                      isStreaming: false,
                      sending: false,
                      streamingMessage: null,
                    })
                  }
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

          const state = get()

          // Remove optimistic user message if it still has temp ID
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
            // Message already has real ID - it was successfully created, keep it
            set({
              error: error.message || 'Failed to send message',
              sending: false,
              isStreaming: false,
              streamingMessage: null,
            })
          }
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
          loadingConversationId: null,
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
