import { savePanelSnapshotForConversation } from '@/modules/chat/core/stores/chat'
import type { ChatSet, ChatInitialState, ChatState } from '@/modules/chat/core/stores/chat'

export default (set: ChatSet, getRaw: () => ChatInitialState) => {
  const get = getRaw as unknown as () => ChatState
  return async () => {
      set(state => ({
        rightPanel: {
          ...state.rightPanel,
          tabs: [],
          activeId: null,
          mobileDrawerOpen: false,
        },
      }))
      const { conversation } = get()
      if (conversation) {
        savePanelSnapshotForConversation(conversation.id, [], null)
      }
    }
}
