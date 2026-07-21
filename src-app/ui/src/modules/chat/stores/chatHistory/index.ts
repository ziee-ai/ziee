import { defineStore, registerLazyStore } from '@ziee/framework/store-kit'
import { chatHistoryState, type ChatHistoryState } from './state'
import type { Actions } from './actions.gen'

const ChatHistoryDef = defineStore<ChatHistoryState, Actions>('ChatHistory', {
  immer: true,
  state: chatHistoryState,
  actions: import.meta.glob('./actions/*.ts'),
  init: ({ on, set, actions }) => {
    on('conversation.created', event => {
      const { conversation } = event.data
      set(draft => {
        // Convert Conversation to ConversationResponse by adding message_count.
        const conversationResponse: ChatHistoryState['conversations'][number] = {
          ...conversation,
          message_count: 0,
        }
        // recentConversations must always reflect the true most-recent list, so
        // prepend there regardless of the current view. Do NOT truncate: the
        // sidebar is now infinite-scroll paged, so already-loaded older pages
        // must survive a new-chat prepend (the old `.slice(0,20)` would drop
        // them). Bump `recentTotal` only when this is genuinely a new id.
        const alreadyInRecent = draft.recentConversations.some(
          c => c.id === conversation.id,
        )
        draft.recentConversations = [
          conversationResponse,
          ...draft.recentConversations.filter(c => c.id !== conversation.id),
        ]
        if (!alreadyInRecent) {
          draft.recentTotal = draft.recentTotal + 1
          // Re-anchor the page cursor to the grown length (same as syncRecentFront
          // and the delete paths). A front-prepend shifts the server offsets, so
          // WITHOUT this, ≥limit accumulated local creates would make the next
          // loadMoreRecent fetch a fully-overlapping page → noProgress → older
          // pages wrongly stranded.
          draft.recentPage = Math.floor(
            draft.recentConversations.length / draft.limit,
          )
        }
        // The main `conversations` list may be a FILTERED (search) or non-recent
        // SORTED view. A brand-new empty conversation won't match a content
        // search and has no defined position under a non-recent sort, so only
        // optimistically insert it (and bump the total) in the unfiltered,
        // default-sort view; otherwise leave the result set to the next load.
        if (!draft.searchQuery.trim() && draft.sort === 'recent') {
          draft.conversations.unshift(conversationResponse)
          draft.total = draft.total + 1
        }
      })
    })
    on('conversation.titleUpdated', event => {
      const { conversationId, title } = event.data
      set(draft => {
        const updateTitle = (conv: ChatHistoryState['conversations'][number]) => {
          if (conv.id === conversationId) conv.title = title
        }
        draft.conversations.forEach(updateTitle)
        draft.recentConversations.forEach(updateTitle)
      })
    })
    // Cross-device sync: notify-and-refetch — the event carries only
    // {action, id}, so reload the first page or drop it on delete.
    on('sync:conversation', async event => {
      const { action, id } = event.data
      if (action === 'delete') {
        set(draft => {
          // Only adjust `total` if the deleted conversation was actually in the
          // current (possibly search-FILTERED) result set. A cross-device delete
          // of a conversation that doesn't match the active search must not
          // decrement the filtered total (which would desync "Showing X of N"
          // and hasMore/Load-More).
          const wasPresent = draft.conversations.some(c => c.id === id)
          const wasInRecent = draft.recentConversations.some(c => c.id === id)
          draft.conversations = draft.conversations.filter(c => c.id !== id)
          draft.recentConversations = draft.recentConversations.filter(c => c.id !== id)
          // Prune the selection too, so a still-selected row can't be
          // double-counted by a later bulkDelete after a cross-device delete.
          draft.selectedIds.delete(id)
          if (wasPresent) draft.total = Math.max(0, draft.total - 1)
          if (wasInRecent) {
            draft.recentTotal = Math.max(0, draft.recentTotal - 1)
            draft.recentPage = Math.floor(
              draft.recentConversations.length / draft.limit,
            )
          }
        })
        // Cross-device delete that emptied the loaded list → refill (see above).
        void actions.refillRecentIfEmptied()
      } else {
        // Refetch the history list (page 1) and MERGE-prepend the sidebar's new
        // rows (preserving its accumulated infinite-scroll pages).
        await actions.loadConversations(1)
        await actions.syncRecentFront()
      }
    })
    // On (re)connect, resync to cover anything missed offline. A full page-1
    // replace of the recent list is correct here (fresh view after a gap).
    on('sync:reconnect', () => {
      void actions.loadConversations(1)
      void actions.loadRecentConversations(1)
    })
  },
})

export const ChatHistory = registerLazyStore(ChatHistoryDef)
export const useChatHistoryStore = ChatHistoryDef.store

// Re-export types so existing imports keep working.
export type { ConversationSort } from './state'
