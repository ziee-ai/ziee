import { ApiClient } from '@/api-client'
import { toOrderedMap } from '@/modules/chat/core/stores/messageWindow'
import { MESSAGE_PAGE_SIZE } from '@/modules/chat/core/stores/chat'
import type { ChatSet, ChatInitialState } from '@/modules/chat/core/stores/chat'

export default (set: ChatSet, _getRaw: () => ChatInitialState) => {
  return async (id: string) => {
      set({ loading: true, error: null })
      try {
        // Newest page (tail): no cursor. Resets the window.
        const page = await ApiClient.Message.getHistory({
          id,
          limit: MESSAGE_PAGE_SIZE,
        })
        set({
          messages: toOrderedMap(page.messages),
          hasMoreBefore: page.has_more_before,
          hasMoreAfter: page.has_more_after,
          loadingOlder: false,
          loadingNewer: false,
          loading: false,
        })
      } catch (error: any) {
        set({
          error: error.message || 'Failed to load messages',
          loading: false,
        })
      }
    }
}
