import type { ChatSet, ChatInitialState, ChatState } from '@/modules/chat/core/stores/Chat.store'

export default (set: ChatSet, getRaw: () => ChatInitialState) => {
  const get = getRaw as unknown as () => ChatState
  return async (branchId: string) => {
      const level = get().pendingBranchForkLevel
      const newLevels = new Map(get().branchForkLevels)
      newLevels.set(branchId, level ?? 'user')
      set({ branchForkLevels: newLevels, pendingBranchForkLevel: null })
    }
}
