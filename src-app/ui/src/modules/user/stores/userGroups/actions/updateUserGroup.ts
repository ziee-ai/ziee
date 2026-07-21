import { ApiClient } from '@/api-client'
import type { Group, UpdateGroupRequest } from '@/api-client/types'
import type { UserGroupsGet, UserGroupsSet } from '../state'
import { emitGroupUpdated } from '@/modules/user/events'

export default (set: UserGroupsSet, get: UserGroupsGet) =>
  async (
    id: string,
    data: UpdateGroupRequest,
  ): Promise<Group | undefined> => {
    if (get().updating) return
    try {
      set({ updating: true, error: null })
      const group = await ApiClient.UserGroup.update({ group_id: id, ...data })
      try {
        await emitGroupUpdated(group)
      } catch (eventError) {
        console.error('Failed to emit group updated event:', eventError)
      }
      set({ updating: false })
      return group
    } catch (error) {
      set({
        error: error instanceof Error ? error.message : 'Failed to update group',
        updating: false,
      })
      throw error
    }
  }
