import { ApiClient } from '@/api-client'
import { type ConversationResponse, Permissions } from '@/api-client/types'
import { hasPermissionNow } from '@/core/permissions'
import { defineStore } from '@/core/store-kit'
import { createStoreProxy } from '@/core/stores'

/**
 * ChatHistory Store — conversation list, search, pagination, bulk operations.
 */
export const ChatHistory = defineStore('ChatHistory', {
  immer: true,
  state: {
    conversations: [] as ConversationResponse[],
    recentConversations: [] as ConversationResponse[],
    // Pagination
    page: 1,
    limit: 20,
    total: 0,
    hasMore: false,
    // Search state
    searchQuery: '',
    filteredConversations: [] as ConversationResponse[],
    // Selection
    selectedIds: new Set<string>(),
    // Loading states
    loading: false,
    loadingMore: false,
    deleting: false,
    error: null as string | null,
    isInitialized: false,
  },
  actions: (set, get) => {
    const loadConversations = async (page?: number) => {
      // Permission-gate the shell-eager-load fetch: the sidebar's recent-
      // conversations widget accesses this store on every render.
      if (!hasPermissionNow(Permissions.ConversationsRead)) return
      const state = get()
      const targetPage = page ?? state.page
      if (state.loading || state.loadingMore) return
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
        const pageItems = response.conversations
        set(draft => {
          if (targetPage === 1) {
            // First page - replace all.
            draft.conversations = pageItems
            draft.recentConversations = pageItems.slice(0, 20)
          } else {
            // Subsequent pages - append.
            draft.conversations = [...draft.conversations, ...pageItems]
          }
          draft.page = targetPage
          draft.hasMore = draft.conversations.length < response.total
          draft.total = response.total
          draft.loading = false
          draft.loadingMore = false
          draft.isInitialized = true
          if (draft.searchQuery) {
            draft.filteredConversations = draft.conversations.filter(conv =>
              conv.title?.toLowerCase().includes(draft.searchQuery.toLowerCase()),
            )
          }
        })
      } catch (error) {
        console.error('[ChatHistory] Failed to load conversations:', error)
        set({ error: 'Failed to load conversations', loading: false, loadingMore: false })
      }
    }
    return {
      loadConversations,
      loadNextPage: async () => {
        const state = get()
        if (!state.hasMore || state.loadingMore) return
        await loadConversations(state.page + 1)
      },
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
      deleteConversation: async (id: string) => {
        set({ deleting: true, error: null })
        try {
          await ApiClient.Conversation.delete({ id })
          set(draft => {
            draft.conversations = draft.conversations.filter(conv => conv.id !== id)
            draft.recentConversations = draft.recentConversations.filter(conv => conv.id !== id)
            draft.filteredConversations = draft.filteredConversations.filter(conv => conv.id !== id)
            draft.selectedIds.delete(id)
            draft.total = draft.conversations.length
            draft.deleting = false
          })
          // Broadcast deletion so other widgets drop the row (closes audit F5).
          // Import-late to avoid a cycle through `@/core/stores`.
          const { Stores } = await import('@/core/stores')
          await Stores.EventBus.emit({ type: 'conversation.deleted', data: { conversationId: id } })
        } catch (error) {
          console.error('[ChatHistory] Failed to delete conversation:', error)
          set({ error: 'Failed to delete conversation', deleting: false })
          throw error
        }
      },
      bulkDelete: async () => {
        const state = get()
        if (state.selectedIds.size === 0) return
        set({ deleting: true, error: null })
        try {
          await Promise.all(
            Array.from(state.selectedIds).map(id => ApiClient.Conversation.delete({ id })),
          )
          set(draft => {
            const selectedIds = Array.from(draft.selectedIds)
            draft.conversations = draft.conversations.filter(c => !selectedIds.includes(c.id))
            draft.recentConversations = draft.recentConversations.filter(
              c => !selectedIds.includes(c.id),
            )
            draft.filteredConversations = draft.filteredConversations.filter(
              c => !selectedIds.includes(c.id),
            )
            draft.selectedIds.clear()
            draft.total = draft.conversations.length
            draft.deleting = false
          })
        } catch (error) {
          console.error('[ChatHistory] Failed to bulk delete conversations:', error)
          set({ error: 'Failed to delete selected conversations', deleting: false })
          throw error
        }
      },
      toggleSelection: (id: string) => {
        set(draft => {
          if (draft.selectedIds.has(id)) draft.selectedIds.delete(id)
          else draft.selectedIds.add(id)
        })
      },
      selectAll: () => {
        set(draft => {
          const visible = draft.searchQuery ? draft.filteredConversations : draft.conversations
          visible.forEach(conv => {
            draft.selectedIds.add(conv.id)
          })
        })
      },
      deselectAll: () => {
        set(draft => {
          draft.selectedIds.clear()
        })
      },
      updateConversationTitle: async (id: string, title: string) => {
        try {
          await ApiClient.Conversation.update({ id, title })
          set(draft => {
            const updateTitle = (conv: ConversationResponse) => {
              if (conv.id === id) conv.title = title
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
      refreshConversations: async () => {
        await loadConversations(1)
      },
    }
  },
  init: ({ on, set, actions }) => {
    on('conversation.created', event => {
      const { conversation } = event.data
      set(draft => {
        // Convert Conversation to ConversationResponse by adding message_count.
        const conversationResponse: ConversationResponse = { ...conversation, message_count: 0 }
        draft.conversations.unshift(conversationResponse)
        draft.recentConversations = draft.conversations.slice(0, 20)
        draft.total = draft.conversations.length
      })
    })
    on('conversation.titleUpdated', event => {
      const { conversationId, title } = event.data
      set(draft => {
        const updateTitle = (conv: ConversationResponse) => {
          if (conv.id === conversationId) conv.title = title
        }
        draft.conversations.forEach(updateTitle)
        draft.recentConversations.forEach(updateTitle)
        draft.filteredConversations.forEach(updateTitle)
      })
    })
    on('conversation.messageCountChanged', event => {
      const { conversationId, messageCount } = event.data
      set(draft => {
        const update = (conv: ConversationResponse) => {
          if (conv.id === conversationId) conv.message_count = messageCount
        }
        draft.conversations.forEach(update)
        draft.recentConversations.forEach(update)
        draft.filteredConversations.forEach(update)
      })
    })
    // Cross-device sync: notify-and-refetch — the event carries only
    // {action, id}, so reload the first page or drop it on delete.
    on('sync:conversation', async event => {
      const { action, id } = event.data
      if (action === 'delete') {
        set(draft => {
          draft.conversations = draft.conversations.filter(c => c.id !== id)
          draft.recentConversations = draft.recentConversations.filter(c => c.id !== id)
          draft.filteredConversations = draft.filteredConversations.filter(c => c.id !== id)
          draft.total = draft.conversations.length
        })
      } else {
        await actions.loadConversations(1)
      }
    })
    // On (re)connect, resync to cover anything missed offline.
    on('sync:reconnect', () => void actions.loadConversations(1))
  },
})

export const useChatHistoryStore = ChatHistory.store
export const ChatHistoryStoreProxy = createStoreProxy(useChatHistoryStore)
