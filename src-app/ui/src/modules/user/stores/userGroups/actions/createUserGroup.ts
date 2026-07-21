import { ApiClient } from '@/api-client'
import type { CreateGroupRequest, Group } from '@/api-client/types'
import type { UserGroupsGet, UserGroupsSet } from '../state'
import { emitGroupCreated } from '@/modules/user/events'

export default (set: UserGroupsSet, get: UserGroupsGet) =>
  async (data: CreateGroupRequest): Promise<Group | undefined> => {
    if (get().creating) return
    try {
      set({ creating: true, error: null })
      const group = await ApiClient.UserGroup.create(data)
      // Event handler updates state (no manual state update here).
      try {
        await emitGroupCreated(group)
      } catch (eventError) {
        console.error('Failed to emit group created event:', eventError)
      }
      set({ creating: false })
      return group
    } catch (error) {
      set({
        error: error instanceof Error ? error.message : 'Failed to create group',
        creating: false,
      })
      throw error
    }
  }
