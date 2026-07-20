import { savePanelSnapshotForConversation } from '@/modules/chat/core/stores/chat'
import type { ChatSet, ChatInitialState, ChatState, PanelRendererMap, PanelType, RightPanelTab } from '@/modules/chat/core/stores/chat'

export default (set: ChatSet, getRaw: () => ChatInitialState) => {
  const get = getRaw as unknown as () => ChatState
  return async <T extends PanelType>(id: string, data: PanelRendererMap[T]) => {
      set(state => {
        const idx = state.rightPanel.tabs.findIndex(t => t.id === id)
        if (idx === -1) return state
        const tabs = state.rightPanel.tabs.slice()
        tabs[idx] = { ...tabs[idx], data: data as RightPanelTab['data'] }
        return { rightPanel: { ...state.rightPanel, tabs } }
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
