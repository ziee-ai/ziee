import type { ChatSet, ChatInitialState } from '@/modules/chat/core/stores/chat'

export default (set: ChatSet, _getRaw: () => ChatInitialState) => {
  return async (forkMessageId: string) => {
      set(state => {
        const sorted = [...state.messages.values()].sort(
          (a, b) =>
            new Date(a.created_at).getTime() - new Date(b.created_at).getTime(),
        )
        const forkIndex = sorted.findIndex(m => m.id === forkMessageId)
        if (forkIndex === -1) return {}
        const newMessages = new Map(state.messages)
        sorted.slice(forkIndex).forEach(m => newMessages.delete(m.id))
        return { messages: newMessages }
      })
    }
}
