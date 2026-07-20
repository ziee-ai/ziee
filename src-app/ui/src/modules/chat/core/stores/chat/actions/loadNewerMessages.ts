import { ApiClient } from '@/api-client'
import { appendWindow, lastMessageId } from '@/modules/chat/core/stores/messageWindow'
import { MESSAGE_PAGE_SIZE } from '@/modules/chat/core/stores/chat'
import type { ChatSet, ChatInitialState, ChatState } from '@/modules/chat/core/stores/chat'

export default (set: ChatSet, getRaw: () => ChatInitialState) => {
  const get = getRaw as unknown as () => ChatState
  return async () => {
      const state = get()
      const conversationId = state.conversation?.id
      // Re-entrancy guard (`loadingNewer`) mirrors `loadingOlder`: the bottom
      // sentinel can fire repeatedly, so drop overlapping same-cursor fetches.
      if (
        !conversationId ||
        !state.hasMoreAfter ||
        state.isStreaming ||
        state.loadingNewer
      ) {
        return
      }
      const newestId = lastMessageId(state.messages)
      if (!newestId) return

      set({ loadingNewer: true })
      try {
        const page = await ApiClient.Message.getHistory({
          id: conversationId,
          after: newestId,
          limit: MESSAGE_PAGE_SIZE,
        })
        if (get().conversation?.id !== conversationId) return
        set(s => ({
          messages: appendWindow(s.messages, page.messages),
          hasMoreAfter: page.has_more_after,
          loadingNewer: false,
        }))
        await get().computeForkPoints()
      } catch (error: any) {
        if (get().conversation?.id === conversationId) {
          set({
            error: error.message || 'Failed to load newer messages',
            loadingNewer: false,
          })
        }
      }
    }
}
