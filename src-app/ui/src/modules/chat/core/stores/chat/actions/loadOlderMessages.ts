import { ApiClient } from '@/api-client'
import { firstMessageId, prependWindow } from '@/modules/chat/core/stores/messageWindow'
import { MESSAGE_PAGE_SIZE } from '@/modules/chat/core/stores/chat'
import type { ChatSet, ChatInitialState, ChatState } from '@/modules/chat/core/stores/chat'

export default (set: ChatSet, getRaw: () => ChatInitialState) => {
  const get = getRaw as unknown as () => ChatState
  return async () => {
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
    }
}
