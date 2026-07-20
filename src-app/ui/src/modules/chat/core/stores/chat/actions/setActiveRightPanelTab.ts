import type { ChatSet, ChatInitialState } from '@/modules/chat/core/stores/Chat.store'

export default (set: ChatSet, _getRaw: () => ChatInitialState) => {
  return async (id: string) => {
      set(state => {
        if (!state.rightPanel.tabs.some(t => t.id === id)) return state
        return { rightPanel: { ...state.rightPanel, activeId: id } }
      })
    }
}
