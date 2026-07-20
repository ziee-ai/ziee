import { savePanelSnapshotForConversation } from '@/modules/chat/core/stores/Chat.store'
import type { ChatSet, ChatInitialState, ChatState } from '@/modules/chat/core/stores/Chat.store'

export default (set: ChatSet, getRaw: () => ChatInitialState) => {
  const get = getRaw as unknown as () => ChatState
  return async (id: string) => {
      set(state => {
        const tabs = state.rightPanel.tabs.filter(t => t.id !== id)
        let activeId = state.rightPanel.activeId
        if (activeId === id) {
          const closedIndex = state.rightPanel.tabs.findIndex(t => t.id === id)
          const next = tabs[closedIndex] ?? tabs[closedIndex - 1] ?? null
          activeId = next?.id ?? null
        }
        const mobileDrawerOpen =
          tabs.length > 0 ? state.rightPanel.mobileDrawerOpen : false
        return {
          rightPanel: { ...state.rightPanel, tabs, activeId, mobileDrawerOpen },
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
