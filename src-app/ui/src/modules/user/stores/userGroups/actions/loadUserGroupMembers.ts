import { ApiClient } from '@/api-client'
import type { GroupMember, UserGroupsGet, UserGroupsSet } from '../state'
import { pendingMemberRequests } from '../state'

export default (set: UserGroupsSet, _get: UserGroupsGet) =>
  async (groupId: string) => {
    // Await an in-flight fetch for this group so the caller reads fresh data
    // rather than a stale false-negative membership check.
    const inFlight = pendingMemberRequests.get(groupId)
    if (inFlight) {
      await inFlight
      return
    }
    try {
      set({ loadingGroupMembers: true, error: null, currentGroupId: groupId })
      const promise = ApiClient.UserGroup.getMembers({
        group_id: groupId,
        page: 1,
        per_page: 50,
      }).then(response => {
        set({
          currentGroupMembers: (Array.isArray(response.users)
            ? response.users
            : []
          ).map(
            (u: {
              id: string
              username: string
              email: string
              is_active: boolean
            }) =>
              ({
                id: u.id,
                username: u.username,
                email: u.email,
                is_active: u.is_active,
                joined_at: new Date().toISOString(),
              } satisfies GroupMember),
          ),
          loadingGroupMembers: false,
        })
      })
      pendingMemberRequests.set(groupId, promise)
      await promise
    } catch (error) {
      set({
        error: error instanceof Error ? error.message : 'Failed to load group members',
        loadingGroupMembers: false,
      })
      throw error
    } finally {
      pendingMemberRequests.delete(groupId)
    }
  }
