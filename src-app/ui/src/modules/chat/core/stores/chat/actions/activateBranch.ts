import { ApiClient } from '@/api-client'
import type { ChatSet, ChatInitialState, ChatState } from '@/modules/chat/core/stores/chat'

export default (set: ChatSet, getRaw: () => ChatInitialState) => {
  const get = getRaw as unknown as () => ChatState
  return async (conversationId: string, branchId: string) => {
      await ApiClient.Branch.activate({
        id: conversationId,
        branch_id: branchId,
      })

      set(state => ({
        conversation: state.conversation
          ? { ...state.conversation, active_branch_id: branchId }
          : null,
      }))

      await get().loadMessages(conversationId)

      const { branches } = get()
      if (!branches.find(b => b.id === branchId)) {
        await get().loadBranches(conversationId)
      } else {
        await get().computeForkPoints()
      }
    }
}
