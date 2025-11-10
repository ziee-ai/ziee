import { create } from 'zustand'
import { subscribeWithSelector } from 'zustand/middleware'
import { ApiClient } from '@/api-client'
import type {
  Group,
  CreateGroupRequest,
  UpdateGroupRequest,
} from '@/api-client/types'
import {
  emitGroupCreated,
  emitGroupUpdated,
  emitGroupDeleted,
  emitGroupMemberAdded,
  emitGroupMemberRemoved,
} from '../events'
import { Stores } from '@/core/stores'

interface GroupMember {
  id: string
  username: string
  email: string
  is_active: boolean
  joined_at: string
}

interface UserGroupsState {
  // Data
  groups: Group[]
  currentGroupMembers: GroupMember[]
  total: number
  currentPage: number
  pageSize: number
  isInitialized: boolean
  currentGroupId: string | null

  // Loading states
  loading: boolean
  loadingGroups: boolean
  loadingGroupMembers: boolean
  creating: boolean
  updating: boolean
  deleting: boolean

  // Error state
  error: string | null

  // Actions
  loadUserGroups: (page?: number, pageSize?: number) => Promise<void>
  createUserGroup: (data: CreateGroupRequest) => Promise<Group | undefined>
  updateUserGroup: (id: string, data: UpdateGroupRequest) => Promise<Group | undefined>
  deleteUserGroup: (id: string) => Promise<void>
  loadUserGroupMembers: (groupId: string) => Promise<void>
  assignUserToUserGroup: (userId: string, groupId: string) => Promise<void>
  removeUserFromUserGroup: (userId: string, groupId: string) => Promise<void>
  clearError: () => void

  __init__: {
    __store__?: () => void
    groups: () => Promise<void>
  }

  __destroy__?: () => void
}

export const useUserGroupsStore = create<UserGroupsState>()(
  subscribeWithSelector(
    (set, get): UserGroupsState => ({
      // Initial state
      groups: [],
      currentGroupMembers: [],
      total: 0,
      currentPage: 1,
      pageSize: 10,
      isInitialized: false,
      currentGroupId: null,
      loading: false,
      loadingGroups: false,
      loadingGroupMembers: false,
      creating: false,
      updating: false,
      deleting: false,
      error: null,

      // Actions
      loadUserGroups: async (page?: number, pageSize?: number) => {
        try {
          const currentState = get()
          const requestPage = page || currentState.currentPage
          const requestPageSize = pageSize || currentState.pageSize

          // Skip if already initialized and loading first page without explicit page parameter
          if (currentState.isInitialized && currentState.loadingGroups && !page) {
            return
          }

          set({ loadingGroups: true, error: null })

          const response = await ApiClient.UserGroup.list({
            page: requestPage,
            per_page: requestPageSize,
          })

          set({
            groups: response.groups,
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
      },

      createUserGroup: async (data: CreateGroupRequest) => {
        const state = get()
        if (state.creating) {
          return
        }

        try {
          set({ creating: true, error: null })

          const group = await ApiClient.UserGroup.create(data)

          // Emit event after successful API call
          // Event handler will update state (no manual state update here)
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

      updateUserGroup: async (id: string, data: UpdateGroupRequest) => {
        const state = get()
        if (state.updating) {
          return
        }

        try {
          set({ updating: true, error: null })

          const group = await ApiClient.UserGroup.update({
            group_id: id,
            ...data,
          })

          // Emit event after successful API call
          // Event handler will update state (no manual state update here)
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
        const state = get()
        if (state.deleting) {
          return
        }

        try {
          set({ deleting: true, error: null })

          await ApiClient.UserGroup.delete({
            group_id: id,
          })

          // Emit event after successful API call
          // Event handler will update state (no manual state update here)
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

      loadUserGroupMembers: async (groupId: string) => {
        try {
          const currentState = get()

          // Skip if already loading members for the same group
          if (
            currentState.loadingGroupMembers &&
            currentState.currentGroupId === groupId
          ) {
            return
          }

          set({
            loadingGroupMembers: true,
            error: null,
            currentGroupId: groupId,
          })

          const response = await ApiClient.UserGroup.getMembers({
            group_id: groupId,
            page: 1,
            per_page: 50,
          })

          set({
            currentGroupMembers: response.users.map(u => ({
              id: u.id,
              username: u.username,
              email: u.email,
              is_active: u.is_active,
              joined_at: new Date().toISOString(),
            })),
            loadingGroupMembers: false,
          })
        } catch (error) {
          set({
            error:
              error instanceof Error ? error.message : 'Failed to load group members',
            loadingGroupMembers: false,
          })
          throw error
        }
      },

      assignUserToUserGroup: async (userId: string, groupId: string) => {
        const state = get()
        if (state.updating) {
          return
        }

        try {
          set({ updating: true, error: null })

          await ApiClient.UserGroup.assignUser({
            user_id: userId,
            group_id: groupId,
          })

          // Emit event after successful API call
          try {
            await emitGroupMemberAdded(groupId, userId)
          } catch (eventError) {
            console.error('Failed to emit group member added event:', eventError)
          }

          // Reload group members if we're viewing this group
          if (state.currentGroupId === groupId) {
            await get().loadUserGroupMembers(groupId)
          }

          set({ updating: false })
        } catch (error) {
          set({
            error:
              error instanceof Error
                ? error.message
                : 'Failed to assign user to group',
            updating: false,
          })
          throw error
        }
      },

      removeUserFromUserGroup: async (userId: string, groupId: string) => {
        const state = get()
        if (state.updating) {
          return
        }

        try {
          set({ updating: true, error: null })

          await ApiClient.UserGroup.removeUser({
            user_id: userId,
            group_id: groupId,
          })

          // Emit event after successful API call
          // Event handler will update state (no manual state update here)
          try {
            await emitGroupMemberRemoved(groupId, userId)
          } catch (eventError) {
            console.error('Failed to emit group member removed event:', eventError)
          }

          set({ updating: false })
        } catch (error) {
          set({
            error:
              error instanceof Error
                ? error.message
                : 'Failed to remove user from group',
            updating: false,
          })
          throw error
        }
      },

      clearError: () => {
        set({ error: null })
      },

      __init__: {
        __store__: () => {
          const eventBus = Stores.EventBus
          const GROUP = 'UserGroupsStore'

          // Subscribe to group.created
          eventBus.on('group.created', async event => {
            const { group } = event.data
            set(state => ({
              groups: [...state.groups, group],
              total: state.total + 1,
            }))
          }, GROUP)

          // Subscribe to group.updated
          eventBus.on('group.updated', async event => {
            const { group } = event.data
            set(state => ({
              groups: state.groups.map(g => (g.id === group.id ? group : g)),
            }))
          }, GROUP)

          // Subscribe to group.deleted
          eventBus.on('group.deleted', async event => {
            const { groupId } = event.data
            set(state => ({
              groups: state.groups.filter(g => g.id !== groupId),
              total: state.total - 1,
            }))
          }, GROUP)

          // Subscribe to group.member_added
          eventBus.on('group.member_added', async event => {
            const { groupId } = event.data
            const state = get()
            // If currentGroupId matches, reload group members
            if (state.currentGroupId === groupId) {
              await get().loadUserGroupMembers(groupId)
            }
          }, GROUP)

          // Subscribe to group.member_removed
          eventBus.on('group.member_removed', async event => {
            const { groupId, userId } = event.data
            const state = get()
            // If currentGroupId matches, remove member from currentGroupMembers
            if (state.currentGroupId === groupId) {
              set(state => ({
                currentGroupMembers: state.currentGroupMembers.filter(
                  m => m.id !== userId,
                ),
              }))
            }
          }, GROUP)

          // Subscribe to user.deleted
          eventBus.on('user.deleted', async event => {
            const { userId } = event.data
            set(state => ({
              currentGroupMembers: state.currentGroupMembers.filter(
                m => m.id !== userId,
              ),
            }))
          }, GROUP)
        },
        groups: () => get().loadUserGroups(),
      },

      __destroy__: () => {
        Stores.EventBus.removeGroupListeners('UserGroupsStore')
      },
    }),
  ),
)
