import { ApiClient } from '@/api-client'
import type { UserGroupsGet, UserGroupsSet } from '../state'
import { emitGroupDeleted } from '@/modules/user/events'

export default (set: UserGroupsSet, get: UserGroupsGet) =>
  async (id: string) => {
    if (get().deleting) return
    try {
      set({ deleting: true, error: null })
      await ApiClient.UserGroup.delete({ group_id: id })
      try {
        await emitGroupDeleted(id)
      } catch (eventError) {
        console.error('Failed to emit group deleted event:', eventError)
      }
      set({ deleting: false })
    } catch (error) {
      set({
        error: error instanceof Error ? error.message : 'Failed to delete group',
        deleting: false,
      })
      throw error
    }
  }
