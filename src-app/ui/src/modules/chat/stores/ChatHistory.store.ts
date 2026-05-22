import { create } from 'zustand'
import { subscribeWithSelector } from 'zustand/middleware'
import { immer } from 'zustand/middleware/immer'
import { createStoreProxy } from '@/core/stores'
import { ApiClient } from '@/api-client'
import type { ConversationResponse } from '@/api-client/types'

/**
 * ChatHistory Store
 * Manages conversation list, search, pagination, and bulk operations
 */
interface ChatHistoryStore {
  // Data
  conversations: ConversationResponse[]
  recentConversations: ConversationResponse[]

  // Pagination
  page: number
  limit: number
  total: number
  hasMore: boolean

  // Search state
  searchQuery: string
  filteredConversations: ConversationResponse[]

  // Selection
  selectedIds: Set<string>

  // Loading states
  loading: boolean
  loadingMore: boolean
  deleting: boolean
  error: string | null

  // Initialization
  isInitialized: boolean

  // Actions
  loadConversations: (page?: number) => Promise<void>
  loadNextPage: () => Promise<void>
  setSearchQuery: (query: string) => void
  deleteConversation: (id: string) => Promise<void>
  bulkDelete: () => Promise<void>
  toggleSelection: (id: string) => void
  selectAll: () => void
  deselectAll: () => void
  updateConversationTitle: (id: string, title: string) => Promise<void>
  refreshConversations: () => Promise<void>
}

export const useChatHistoryStore = create<ChatHistoryStore>()(
  subscribeWithSelector(
    immer((set, get) => ({
      // Initial state
      conversations: [],
      recentConversations: [],
      page: 1,
      limit: 20,
      total: 0,
      hasMore: false,
      searchQuery: '',
      filteredConversations: [],
      selectedIds: new Set(),
      loading: false,
      loadingMore: false,
      deleting: false,
      error: null,
      isInitialized: false,

      /**
       * Load conversations with pagination
       */
      loadConversations: async (page?: number) => {
        const state = get()
        const targetPage = page ?? state.page

        // Prevent loading if already loading
        if (state.loading || state.loadingMore) {
          return
        }

        set({
          loading: targetPage === 1,
          loadingMore: targetPage > 1,
          error: null,
        })

        try {
          const response = await ApiClient.Conversation.list({
            page: targetPage,
            limit: state.limit,
          })

          set(draft => {
            if (targetPage === 1) {
              // First page - replace all conversations
              draft.conversations = response
              draft.recentConversations = response.slice(0, 20)
            } else {
              // Subsequent pages - append conversations
              draft.conversations = [...draft.conversations, ...response]
            }

            draft.page = targetPage
            draft.hasMore = response.length === state.limit
            draft.total = draft.conversations.length
            draft.loading = false
            draft.loadingMore = false
            draft.isInitialized = true

            // Update filtered conversations if searching
            if (draft.searchQuery) {
              draft.filteredConversations = draft.conversations.filter(conv =>
                conv.title?.toLowerCase().includes(draft.searchQuery.toLowerCase()),
              )
            }
          })
        } catch (error) {
          console.error('[ChatHistory] Failed to load conversations:', error)
          set({
            error: 'Failed to load conversations',
            loading: false,
            loadingMore: false,
          })
        }
      },

      /**
       * Load next page of conversations
       */
      loadNextPage: async () => {
        const state = get()
        if (!state.hasMore || state.loadingMore) {
          return
        }

        await get().loadConversations(state.page + 1)
      },

      /**
       * Set search query and filter conversations
       */
      setSearchQuery: (query: string) => {
        set(draft => {
          draft.searchQuery = query

          if (query.trim()) {
            draft.filteredConversations = draft.conversations.filter(conv =>
              conv.title?.toLowerCase().includes(query.toLowerCase()),
            )
          } else {
            draft.filteredConversations = []
          }
        })
      },

      /**
       * Delete a single conversation
       */
      deleteConversation: async (id: string) => {
        set({ deleting: true, error: null })

        try {
          await ApiClient.Conversation.delete({ id })

          set(draft => {
            draft.conversations = draft.conversations.filter(conv => conv.id !== id)
            draft.recentConversations = draft.recentConversations.filter(
              conv => conv.id !== id,
            )
            draft.filteredConversations = draft.filteredConversations.filter(
              conv => conv.id !== id,
            )
            draft.selectedIds.delete(id)
            draft.total = draft.conversations.length
            draft.deleting = false
          })
        } catch (error) {
          console.error('[ChatHistory] Failed to delete conversation:', error)
          set({
            error: 'Failed to delete conversation',
            deleting: false,
          })
          throw error
        }
      },

      /**
       * Delete all selected conversations
       */
      bulkDelete: async () => {
        const state = get()
        if (state.selectedIds.size === 0) {
          return
        }

        set({ deleting: true, error: null })

        try {
          // Delete all selected conversations in parallel
          await Promise.all(
            Array.from(state.selectedIds).map(id =>
              ApiClient.Conversation.delete({ id }),
            ),
          )

          set(draft => {
            const selectedIds = Array.from(draft.selectedIds)
            draft.conversations = draft.conversations.filter(
              conv => !selectedIds.includes(conv.id),
            )
            draft.recentConversations = draft.recentConversations.filter(
              conv => !selectedIds.includes(conv.id),
            )
            draft.filteredConversations = draft.filteredConversations.filter(
              conv => !selectedIds.includes(conv.id),
            )
            draft.selectedIds.clear()
            draft.total = draft.conversations.length
            draft.deleting = false
          })
        } catch (error) {
          console.error('[ChatHistory] Failed to bulk delete conversations:', error)
          set({
            error: 'Failed to delete selected conversations',
            deleting: false,
          })
          throw error
        }
      },

      /**
       * Toggle conversation selection
       */
      toggleSelection: (id: string) => {
        set(draft => {
          if (draft.selectedIds.has(id)) {
            draft.selectedIds.delete(id)
          } else {
            draft.selectedIds.add(id)
          }
        })
      },

      /**
       * Select all visible conversations
       */
      selectAll: () => {
        set(draft => {
          const visibleConversations =
            draft.searchQuery ? draft.filteredConversations : draft.conversations

          visibleConversations.forEach(conv => {
            draft.selectedIds.add(conv.id)
          })
        })
      },

      /**
       * Deselect all conversations
       */
      deselectAll: () => {
        set(draft => {
          draft.selectedIds.clear()
        })
      },

      /**
       * Update conversation title
       */
      updateConversationTitle: async (id: string, title: string) => {
        try {
          await ApiClient.Conversation.update({
            id,
            title,
          })

          set(draft => {
            const updateTitle = (conv: ConversationResponse) => {
              if (conv.id === id) {
                conv.title = title
              }
            }

            draft.conversations.forEach(updateTitle)
            draft.recentConversations.forEach(updateTitle)
            draft.filteredConversations.forEach(updateTitle)
          })
        } catch (error) {
          console.error('[ChatHistory] Failed to update conversation title:', error)
          set({ error: 'Failed to update conversation title' })
          throw error
        }
      },

      /**
       * Refresh conversations (reload current page)
       */
      refreshConversations: async () => {
        await get().loadConversations(1)
      },

      /**
       * Lifecycle hooks
       */
      __init__: {
        __store__: async () => {
          // Subscribe to conversation.created event
          const { Stores } = await import('@/core/stores')
          Stores.EventBus.on(
            'conversation.created',
            async event => {
              const { conversation } = event.data
              set(draft => {
                // Add to beginning of conversations array
                // Convert Conversation to ConversationResponse by adding message_count
                const conversationResponse: ConversationResponse = {
                  ...conversation,
                  message_count: 0,
                }
                draft.conversations.unshift(conversationResponse)
                // Update recent conversations
                draft.recentConversations = draft.conversations.slice(0, 20)
                // Update total
                draft.total = draft.conversations.length
              })
              console.log('[ChatHistory] Added new conversation:', conversation.id)
            },
            'ChatHistory',
          )

          // Subscribe to conversation.titleUpdated event
          Stores.EventBus.on(
            'conversation.titleUpdated',
            async event => {
              const { conversationId, title } = event.data
              set(draft => {
                const updateTitle = (conv: ConversationResponse) => {
                  if (conv.id === conversationId) {
                    conv.title = title
                  }
                }
                draft.conversations.forEach(updateTitle)
                draft.recentConversations.forEach(updateTitle)
                draft.filteredConversations.forEach(updateTitle)
              })
              console.log('[ChatHistory] Updated conversation title:', conversationId)
            },
            'ChatHistory',
          )

          // Subscribe to conversation.messageCountChanged event
          Stores.EventBus.on(
            'conversation.messageCountChanged',
            async event => {
              const { conversationId, messageCount } = event.data
              set(draft => {
                const update = (conv: ConversationResponse) => {
                  if (conv.id === conversationId) {
                    conv.message_count = messageCount
                  }
                }
                draft.conversations.forEach(update)
                draft.recentConversations.forEach(update)
                draft.filteredConversations.forEach(update)
              })
              console.log('[ChatHistory] Updated message count for:', conversationId, messageCount)
            },
            'ChatHistory',
          )
        },
      },

      /**
       * Cleanup lifecycle hook
       * Called when store is destroyed
       */
      __destroy__: async () => {
        // Unsubscribe from all event listeners for this store
        const { Stores } = await import('@/core/stores')
        Stores.EventBus.removeGroupListeners('ChatHistory')
        console.log('[ChatHistory] Cleaned up event listeners')
      },
    })),
  ),
)

/**
 * Store proxy for declarative access
 */
export const ChatHistoryStoreProxy = createStoreProxy(useChatHistoryStore)
