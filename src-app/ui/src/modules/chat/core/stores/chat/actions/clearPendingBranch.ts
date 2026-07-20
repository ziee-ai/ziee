import type { ChatSet, ChatInitialState } from '@/modules/chat/core/stores/chat'

export default (set: ChatSet, _getRaw: () => ChatInitialState) => {
  return async () => {
      set({
        pendingBranchFromMessageId: null,
        pendingBranchForkLevel: null,
        editingMessage: null,
      })
    }
}
