import { ApiClient } from '@/api-client'
import {
  type CreateGroupRequest,
  type Group,
  Permissions,
  type UpdateGroupRequest,
} from '@/api-client/types'
import { hasPermissionNow } from '@/core/permissions'
import { defineStore } from '@ziee/framework/store-kit'
import {
  emitGroupCreated,
  emitGroupDeleted,
  emitGroupMemberAdded,
  emitGroupMemberRemoved,
  emitGroupUpdated,
} from '@/modules/user/events'

// Tracks in-flight group-member fetch promises so concurrent requests for the
// same group await the in-flight call instead of returning stale data (a false
// negative when a user belongs to 2+ groups).
const pendingMemberRequests = new Map<string, Promise<void>>()

interface GroupMember {
  id: string
  username: string
  email: string
  is_active: boolean
  joined_at: string
}

export const UserGroups = defineStore('UserGroups', {
  state: {
    groups: [] as Group[],
    currentGroupMembers: [] as GroupMember[],
    total: 0,
    currentPage: 1,
    pageSize: 10,
    isInitialized: false,
    currentGroupId: null as string | null,
    loading: false,
    loadingGroups: false,
    loadingGroupMembers: false,
    creating: false,
    updating: false,
    deleting: false,
    error: null as string | null,
  },
  actions: (set, get) => {
    const loadUserGroups = async (page?: number, pageSize?: number) => {
      if (!hasPermissionNow(Permissions.GroupsRead)) return
      try {
        const currentState = get()
        const requestPage = page || currentState.currentPage
        const requestPageSize = pageSize || currentState.pageSize
        // Skip if already initialized and loading first page without explicit page.
        if (currentState.isInitialized && currentState.loadingGroups && !page) return
        set({ loadingGroups: true, error: null })
        const response = await ApiClient.UserGroup.list({
          page: requestPage,
          per_page: requestPageSize,
        })
        set({
          // Guard: a malformed/edge response must not set `groups` undefined —
          // consumers (UserGroupsDrawer) map over it unconditionally.
          groups: Array.isArray(response.groups) ? response.groups : [],
          total: response.total,
          currentPage: response.page,
          pageSize: response.per_page,
          isInitialized: true,
          loadingGroups: false,
        })
      } catch (error) {
        set({
          error: error instanceof Error ? error.message : 'Failed to load groups',
          loadingGroups: false,
        })
        throw error
      }
    }
    const loadUserGroupMembers = async (groupId: string) => {
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
            ).map(u => ({
              id: u.id,
              username: u.username,
              email: u.email,
              is_active: u.is_active,
              joined_at: new Date().toISOString(),
            })),
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
    return {
      loadUserGroups,
      loadUserGroupMembers,
      createUserGroup: async (data: CreateGroupRequest): Promise<Group | undefined> => {
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
      },
      updateUserGroup: async (
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
      },
      deleteUserGroup: async (id: string) => {
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
      },
      assignUserToUserGroup: async (userId: string, groupId: string) => {
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
          if (state.currentGroupId === groupId) await loadUserGroupMembers(groupId)
          set({ updating: false })
        } catch (error) {
          set({
            error: error instanceof Error ? error.message : 'Failed to assign user to group',
            updating: false,
          })
          throw error
        }
      },
      removeUserFromUserGroup: async (userId: string, groupId: string) => {
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
      },
      clearError: () => {
        set({ error: null })
      },
    }
  },
  init: ({ on, get, set, actions }) => {
    on('group.created', event => {
      set(state => ({ groups: [...state.groups, event.data.group], total: state.total + 1 }))
    })
    on('group.updated', event => {
      set(state => ({
        groups: state.groups.map(g => (g.id === event.data.group.id ? event.data.group : g)),
      }))
    })
    on('group.deleted', event => {
      set(state => ({
        groups: state.groups.filter(g => g.id !== event.data.groupId),
        total: state.total - 1,
      }))
    })
    on('group.member_added', async event => {
      if (get().currentGroupId === event.data.groupId) {
        await actions.loadUserGroupMembers(event.data.groupId)
      }
    })
    on('group.member_removed', event => {
      const { groupId, userId } = event.data
      if (get().currentGroupId === groupId) {
        set(state => ({
          currentGroupMembers: state.currentGroupMembers.filter(m => m.id !== userId),
        }))
      }
    })
    on('user.deleted', event => {
      set(state => ({
        currentGroupMembers: state.currentGroupMembers.filter(m => m.id !== event.data.userId),
      }))
    })
    // Remote sync: loadUserGroups self-gates on GroupsRead.
    const reload = () => void actions.loadUserGroups()
    on('sync:group', reload)
    on('sync:reconnect', reload)
    void actions.loadUserGroups()
  },
})

export const useUserGroupsStore = UserGroups.store
