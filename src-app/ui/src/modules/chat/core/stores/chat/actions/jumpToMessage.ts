import { ApiClient } from '@/api-client'
import { toOrderedMap } from '@/modules/chat/core/stores/messageWindow'
import { MESSAGE_PAGE_SIZE } from '@/modules/chat/core/stores/Chat.store'
import type { ChatSet, ChatInitialState, ChatState } from '@/modules/chat/core/stores/Chat.store'

export default (set: ChatSet, getRaw: () => ChatInitialState) => {
  const get = getRaw as unknown as () => ChatState
  return async (messageId: string): Promise<boolean> => {
      const conversationId = get().conversation?.id
      if (!conversationId) return false

      // Already loaded → no fetch needed; caller scrolls to it.
      if (get().messages.has(messageId)) return true

      try {
        const page = await ApiClient.Message.getHistory({
          id: conversationId,
          around: messageId,
          limit: MESSAGE_PAGE_SIZE,
        })
        if (get().conversation?.id !== conversationId) return false
        // Replace the window with the centered window.
        set({
          messages: toOrderedMap(page.messages),
          hasMoreBefore: page.has_more_before,
          hasMoreAfter: page.has_more_after,
          loadingOlder: false,
          loadingNewer: false,
        })
        await get().computeForkPoints()
        return get().messages.has(messageId)
      } catch (error: any) {
        if (get().conversation?.id === conversationId) {
          set({ error: error.message || 'Failed to jump to message' })
        }
        return false
      }
    }
}
