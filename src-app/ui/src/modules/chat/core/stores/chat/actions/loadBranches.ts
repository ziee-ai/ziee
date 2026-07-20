import { ApiClient } from '@/api-client'

import type { ChatSet, ChatInitialState, ChatState } from '@/modules/chat/core/stores/chat'

export default (set: ChatSet, getRaw: () => ChatInitialState) => {
  const get = getRaw as unknown as () => ChatState
  return async (conversationId: string) => {
      set({ branchesLoading: true })
      try {
        const branches = await ApiClient.Branch.list({ id: conversationId })

        // Seed branchForkLevels from the persisted fork_level on each branch.
        // This ensures computeForkPoints anchors the navigator correctly after page reload,
        // without relying on in-memory state that is lost on refresh.
        const branchForkLevels = new Map(
          branches.map(b => [
            b.id,
            (b.fork_level ?? 'user') as 'user' | 'assistant',
          ]),
        )

        set({ branches, branchForkLevels, branchesLoading: false })
        await get().computeForkPoints()
      } catch (err) {
        console.error('[Chat.store] Failed to load branches:', err)
        set({ branchesLoading: false })
      }
    }
}
