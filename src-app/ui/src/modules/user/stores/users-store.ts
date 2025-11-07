import { create } from 'zustand'
import { subscribeWithSelector } from 'zustand/middleware'
import { ApiClient } from '@/api-client'
import type {
  User,
  CreateUserRequest,
  UpdateUserRequest,
} from '@/api-client/types'
import {
  emitUserCreated,
  emitUserUpdated,
  emitUserDeleted,
} from '../events'
import { Stores } from '@/core/stores'

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
    __store__?: () => void
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

          // Emit event after successful API call
          try {
            await emitUserCreated(user)
          } catch (eventError) {
            console.error('Failed to emit user created event:', eventError)
          }

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

          // Emit event after successful API call
          try {
            await emitUserUpdated(user)
          } catch (eventError) {
            console.error('Failed to emit user updated event:', eventError)
          }

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

          // Emit event after successful API call
          // Find the user and emit update event
          const user = get().users.find(u => u.id === id)
          if (user) {
            try {
              await emitUserUpdated(user)
            } catch (eventError) {
              console.error('Failed to emit user updated event:', eventError)
            }
          }

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

          // Update local state
          let updatedUser: User | undefined
          set(state => ({
            users: state.users.map(u => {
              if (u.id === id) {
                updatedUser = { ...u, is_active: !u.is_active }
                return updatedUser
              }
              return u
            }),
            updating: false,
          }))

          // Emit event after successful API call
          if (updatedUser) {
            try {
              await emitUserUpdated(updatedUser)
            } catch (eventError) {
              console.error('Failed to emit user updated event:', eventError)
            }
          }
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

          // Emit event after successful API call
          try {
            await emitUserDeleted(id)
          } catch (eventError) {
            console.error('Failed to emit user deleted event:', eventError)
          }

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
        __store__: () => {
          const eventBus = Stores.EventBus

          // Subscribe to user.updated
          eventBus.on('user.updated', async event => {
            const { user } = event.data
            set(state => ({
              users: state.users.map(u => (u.id === user.id ? user : u)),
            }))
          })

          // Subscribe to user.deleted
          eventBus.on('user.deleted', async event => {
            const { userId } = event.data
            set(state => ({
              users: state.users.filter(u => u.id !== userId),
              total: state.total - 1,
            }))
          })
        },
        users: () => get().loadUsers(),
      },
    }),
  ),
)
