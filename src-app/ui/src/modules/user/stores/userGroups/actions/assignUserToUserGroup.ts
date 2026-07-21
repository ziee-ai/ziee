import { ApiClient } from '@/api-client'
import type { UserGroupsGet, UserGroupsSet } from '../state'
import { emitGroupMemberAdded } from '@/modules/user/events'
import loadUserGroupMembersFactory from './loadUserGroupMembers'

export default (set: UserGroupsSet, get: UserGroupsGet) =>
  async (userId: string, groupId: string) => {
    const state = get()
    if (state.updating) return
    try {
      set({ updating: true, error: null })
      await ApiClient.UserGroup.assignUser({ user_id: userId, group_id: groupId })
      try {
        await emitGroupMemberAdded(groupId, userId)
      } catch (eventError) {
        console.error('Failed to emit group member added event:', eventError)
      }
      // Reload group members if we're viewing this group.
      if (state.currentGroupId === groupId) {
        const loadMembers = loadUserGroupMembersFactory(set, get)
        await loadMembers(groupId)
      }
      set({ updating: false })
    } catch (error) {
      set({
        error: error instanceof Error ? error.message : 'Failed to assign user to group',
        updating: false,
      })
      throw error
    }
  }
