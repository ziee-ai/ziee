import type { ChatSet, ChatInitialState } from '@/modules/chat/core/stores/Chat.store'

export default (set: ChatSet, _getRaw: () => ChatInitialState) => {
  return async () => set({ error: null })
}
