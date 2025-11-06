import { create } from 'zustand'
import { subscribeWithSelector } from 'zustand/middleware'
import { ApiClient } from '@/api-client'
import type {
  User,
  CreateUserRequest,
  UpdateUserRequest,
  Group,
  CreateGroupRequest,
  UpdateGroupRequest,
} from '@/api-client/types'

// =====================================================
// Users Store
// =====================================================

interface UsersState {
  // Data
  users: User[]
  total: number
  currentPage: number
  pageSize: number
  isInitialized: boolean

  // User registration settings
  userRegistrationEnabled: boolean
  registrationSettingsInitialized: boolean
  loadingRegistrationSettings: boolean

  // Loading states
  loading: boolean
  creating: boolean
  updating: boolean
  deleting: boolean

  // Error state
  error: string | null

  // Actions
  loadUsers: (page?: number, pageSize?: number) => Promise<void>
  createUser: (data: CreateUserRequest) => Promise<User | undefined>
  updateUser: (id: string, data: UpdateUserRequest) => Promise<User | undefined>
  resetUserPassword: (id: string, newPassword: string) => Promise<void>
  toggleUserActiveStatus: (id: string) => Promise<void>
  deleteUser: (id: string) => Promise<void>
  clearError: () => void
  loadUserRegistrationSettings: () => Promise<void>
  updateUserRegistrationSettings: (enabled: boolean) => Promise<void>

  __init__: {
    users: () => Promise<void>
  }
}

export const useUsersStore = create<UsersState>()(
  subscribeWithSelector(
    (set, get): UsersState => ({
      // Initial state
      users: [],
      total: 0,
      currentPage: 1,
      pageSize: 10,
      isInitialized: false,
      userRegistrationEnabled: true,
      registrationSettingsInitialized: false,
      loadingRegistrationSettings: false,
      loading: false,
      creating: false,
      updating: false,
      deleting: false,
      error: null,

      // Actions
      loadUsers: async (page?: number, pageSize?: number) => {
        try {
          const currentState = get()
          const requestPage = page || currentState.currentPage
          const requestPageSize = pageSize || currentState.pageSize

          // Skip if already initialized and loading first page without explicit page parameter
          if (currentState.isInitialized && currentState.loading && !page) {
            return
          }

          set({ loading: true, error: null })

          const response = await ApiClient.User.list({
            page: requestPage,
            per_page: requestPageSize,
          })

          set({
            users: response.users,
            total: response.total,
            currentPage: response.page,
            pageSize: response.per_page,
            isInitialized: true,
            loading: false,
          })
        } catch (error) {
          set({
            error: error instanceof Error ? error.message : 'Failed to load users',
            loading: false,
          })
          throw error
        }
      },

      createUser: async (data: CreateUserRequest) => {
        const state = get()
        if (state.creating) {
          return
        }

        try {
          set({ creating: true, error: null })

          const user = await ApiClient.User.create(data)

          set(state => ({
            users: [...state.users, user],
            total: state.total + 1,
            creating: false,
          }))

          return user
        } catch (error) {
          set({
            error: error instanceof Error ? error.message : 'Failed to create user',
            creating: false,
          })
          throw error
        }
      },

      updateUser: async (id: string, data: UpdateUserRequest) => {
        const state = get()
        if (state.updating) {
          return
        }

        try {
          set({ updating: true, error: null })

          const user = await ApiClient.User.update({
            user_id: id,
            ...data,
          })

          set(state => ({
            users: state.users.map(u => (u.id === id ? user : u)),
            updating: false,
          }))

          return user
        } catch (error) {
          set({
            error: error instanceof Error ? error.message : 'Failed to update user',
            updating: false,
          })
          throw error
        }
      },

      resetUserPassword: async (id: string, newPassword: string) => {
        const state = get()
        if (state.updating) {
          return
        }

        try {
          set({ updating: true, error: null })

          await ApiClient.User.resetPassword({
            user_id: id,
            new_password: newPassword,
          })

          set({ updating: false })
        } catch (error) {
          set({
            error: error instanceof Error ? error.message : 'Failed to reset password',
            updating: false,
          })
          throw error
        }
      },

      toggleUserActiveStatus: async (id: string) => {
        const state = get()
        if (state.updating) {
          return
        }

        try {
          set({ updating: true, error: null })

          await ApiClient.User.toggleActive({
            user_id: id,
          })

          set(state => ({
            users: state.users.map(u =>
              u.id === id ? { ...u, is_active: !u.is_active } : u,
            ),
            updating: false,
          }))
        } catch (error) {
          set({
            error: error instanceof Error ? error.message : 'Failed to toggle user status',
            updating: false,
          })
          throw error
        }
      },

      deleteUser: async (id: string) => {
        const state = get()
        if (state.deleting) {
          return
        }

        try {
          set({ deleting: true, error: null })

          await ApiClient.User.delete({
            user_id: id,
          })

          set(state => ({
            users: state.users.filter(u => u.id !== id),
            total: state.total - 1,
            deleting: false,
          }))
        } catch (error) {
          set({
            error: error instanceof Error ? error.message : 'Failed to delete user',
            deleting: false,
          })
          throw error
        }
      },

      clearError: () => {
        set({ error: null })
      },

      loadUserRegistrationSettings: async () => {
        const state = get()
        if (
          state.registrationSettingsInitialized ||
          state.loadingRegistrationSettings
        ) {
          return
        }
        try {
          set({
            loadingRegistrationSettings: true,
            error: null,
          })

          // TODO: Replace with actual API call when backend endpoint exists
          // const { enabled } = await callAsync('GET /api/users/registration-settings', {})

          set({
            userRegistrationEnabled: true, // Default for now
            registrationSettingsInitialized: true,
            loadingRegistrationSettings: false,
          })
        } catch (error) {
          set({
            error:
              error instanceof Error
                ? error.message
                : 'Failed to load registration settings',
            loadingRegistrationSettings: false,
          })
          throw error
        }
      },

      updateUserRegistrationSettings: async (enabled: boolean) => {
        const state = get()
        if (state.updating) {
          return
        }

        try {
          set({ updating: true, error: null })

          // TODO: Replace with actual API call when backend endpoint exists
          // await callAsync('POST /api/users/registration-settings', { enabled })

          set({
            userRegistrationEnabled: enabled,
            updating: false,
          })
        } catch (error) {
          set({
            error:
              error instanceof Error
                ? error.message
                : 'Failed to update registration settings',
            updating: false,
          })
          throw error
        }
      },

      __init__: {
        users: () => get().loadUsers(),
      },
    }),
  ),
)

// =====================================================
// User Groups Store
// =====================================================

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
    groups: () => Promise<void>
  }
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

          set(state => ({
            groups: [...state.groups, group],
            total: state.total + 1,
            creating: false,
          }))

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

          set(state => ({
            groups: state.groups.map(g => (g.id === id ? group : g)),
            updating: false,
          }))

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

          set(state => ({
            groups: state.groups.filter(g => g.id !== id),
            total: state.total - 1,
            deleting: false,
          }))
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

          // Remove from current group members list
          set(state => ({
            currentGroupMembers: state.currentGroupMembers.filter(
              m => m.id !== userId,
            ),
            updating: false,
          }))
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
        groups: () => get().loadUserGroups(),
      },
    }),
  ),
)

