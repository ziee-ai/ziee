import { ApiClient } from '@/api-client'
import type { UserGroupsGet, UserGroupsSet } from '../state'
import { emitGroupMemberRemoved } from '@/modules/user/events'

export default (set: UserGroupsSet, get: UserGroupsGet) =>
  async (userId: string, groupId: string) => {
    if (get().updating) return
    try {
      set({ updating: true, error: null })
      await ApiClient.UserGroup.removeUser({ user_id: userId, group_id: groupId })
      try {
        await emitGroupMemberRemoved(groupId, userId)
      } catch (eventError) {
        console.error('Failed to emit group member removed event:', eventError)
      }
      set({ updating: false })
    } catch (error) {
      set({
        error: error instanceof Error ? error.message : 'Failed to remove user from group',
        updating: false,
      })
      throw error
    }
  }
