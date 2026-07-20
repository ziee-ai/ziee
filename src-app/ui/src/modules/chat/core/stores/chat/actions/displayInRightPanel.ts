import { savePanelSnapshotForConversation } from '@/modules/chat/core/stores/Chat.store'
import type { ChatSet, ChatInitialState, ChatState, PanelType, RightPanelTab } from '@/modules/chat/core/stores/Chat.store'

export default (set: ChatSet, getRaw: () => ChatInitialState) => {
  const get = getRaw as unknown as () => ChatState
  return async <T extends PanelType>(entry: RightPanelTab<T>) => {
      set(state => {
        const exists = state.rightPanel.tabs.some(t => t.id === entry.id)
        if (exists) {
          return {
            rightPanel: {
              ...state.rightPanel,
              activeId: entry.id,
              mobileDrawerOpen: true,
            },
          }
        }
        return {
          rightPanel: {
            ...state.rightPanel,
            tabs: [...state.rightPanel.tabs, entry as RightPanelTab],
            activeId: entry.id,
            mobileDrawerOpen: true,
          },
        }
      })
      const { rightPanel, conversation } = get()
      if (conversation) {
        savePanelSnapshotForConversation(
          conversation.id,
          rightPanel.tabs,
          rightPanel.activeId,
        )
      }
    }
}
