import { computeChildAnchor, computeParentAnchor } from '@/modules/chat/core/utils/branchAnchor.utils'
import type { ChatSet, ChatInitialState, ChatState } from '@/modules/chat/core/stores/chat'

export default (set: ChatSet, getRaw: () => ChatInitialState) => {
  const get = getRaw as unknown as () => ChatState
  return async () => {
      const state = get()
      const { branches, branchForkLevels } = state
      const conversation = state.conversation

      if (!conversation || branches.length <= 1) {
        set({ forkPoints: new Map() })
        return
      }

      const activeBranchId = conversation.active_branch_id
      const messages = [...state.messages.values()].sort(
        (a, b) =>
          new Date(a.created_at).getTime() - new Date(b.created_at).getTime(),
      )
      const messageIds = new Set(messages.map(m => m.id))

      const forkPoints = new Map<string, string[]>()

      // Group child branches by composite key: `${created_from_message_id}__${forkLevel}`.
      // A user message can be the fork origin for two independent sets of branches —
      // one from Regenerate ('assistant' level) and one from Edit ('user' level).
      // Grouping by both dimensions ensures each produces its own independent navigator.
      const forkGroups = new Map<string, string[]>()
      for (const branch of branches) {
        if (branch.created_from_message_id) {
          const forkLevel = branchForkLevels.get(branch.id) ?? 'user'
          const key = `${branch.created_from_message_id}__${forkLevel}`
          if (!forkGroups.has(key)) {
            forkGroups.set(key, [])
          }
          forkGroups.get(key)!.push(branch.id)
        }
      }

      const currentBranch = branches.find(b => b.id === activeBranchId)

      for (const [groupKey, childBranchIds] of forkGroups) {
        const separatorIdx = groupKey.lastIndexOf('__')
        const forkMsgId = groupKey.slice(0, separatorIdx)
        const forkLevel = groupKey.slice(separatorIdx + 2) as
          | 'user'
          | 'assistant'

        const firstChild = branches.find(b => b.id === childBranchIds[0])
        const parentBranchId = firstChild?.parent_branch_id

        const groupBranchIds = parentBranchId
          ? [parentBranchId, ...childBranchIds]
          : childBranchIds

        const groupBranches = groupBranchIds
          .map(id => branches.find(b => b.id === id))
          .filter(Boolean)
          .sort(
            (a, b) =>
              new Date(a!.created_at).getTime() -
              new Date(b!.created_at).getTime(),
          )
        const sortedGroupIds = groupBranches.map(b => b!.id)

        if (sortedGroupIds.length <= 1) continue

        let anchorMessageId: string | null = null

        if (activeBranchId === parentBranchId) {
          anchorMessageId = computeParentAnchor(
            forkMsgId,
            forkLevel,
            messages,
            messageIds,
          )
        } else if (
          activeBranchId &&
          childBranchIds.includes(activeBranchId) &&
          currentBranch
        ) {
          anchorMessageId = computeChildAnchor(
            activeBranchId,
            currentBranch.created_at,
            messages,
            branchForkLevels,
          )
        }

        if (anchorMessageId) {
          forkPoints.set(anchorMessageId, sortedGroupIds)
        }
      }

      set({ forkPoints })
    }
}
