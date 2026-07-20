import { ApiClient } from '@/api-client'
import { mergeTailWindow, toOrderedMap } from '@/modules/chat/core/stores/messageWindow'
import { MESSAGE_PAGE_SIZE } from '@/modules/chat/core/stores/chat'
import type { ChatSet, ChatInitialState, ChatState } from '@/modules/chat/core/stores/chat'

export default (set: ChatSet, getRaw: () => ChatInitialState) => {
  const get = getRaw as unknown as () => ChatState
  return async (conversationId: string) => {
      try {
        const page = await ApiClient.Message.getHistory({
          id: conversationId,
          limit: MESSAGE_PAGE_SIZE,
        })
        // Only apply to the still-open conversation.
        if (get().conversation?.id !== conversationId) return
        if (get().hasMoreAfter) {
          // The window is anchored MID-conversation (e.g. after an around=
          // jump), so the loaded slice does NOT abut the real tail — a merge
          // would splice the tail on after a gap. Snap to the tail instead.
          set({
            messages: toOrderedMap(page.messages),
            hasMoreBefore: page.has_more_before,
            hasMoreAfter: page.has_more_after,
            loadingOlder: false,
            loadingNewer: false,
          })
        } else {
          // Window already includes the tail: merge so loaded older pages stay
          // and the new turn appends at the bottom.
          set(s => ({
            messages: mergeTailWindow(s.messages, page.messages),
            hasMoreAfter: false,
          }))
        }
      } catch (error: any) {
        if (get().conversation?.id === conversationId) {
          set({ error: error.message || 'Failed to refresh messages' })
        }
      }
    }
}
