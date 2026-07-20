import { ApiClient } from '@/api-client'
import {
  type CreateUserRequest,
  Permissions,
  type UpdateUserRequest,
  type User,
} from '@/api-client/types'
import { hasPermissionNow } from '@/core/permissions'
import { defineStore, registerLazyStore } from '@ziee/framework/store-kit'
import { emitUserCreated, emitUserDeleted, emitUserUpdated } from '@/modules/user/events'

// WHOLE-STORE-LAZY (proof): this store is NOT registered by user/module.tsx.
// It self-registers the moment its chunk is imported (by a direct-handle
// consumer or the lazy Users page), so its whole code — state + actions — rides
// that lazy chunk instead of the eager entry chunk. `Stores.Users` (shim) and
// `import { Users }` return the SAME lifecycle proxy.
const UsersStoreDef = defineStore('Users', {
  state: {
    users: [] as User[],
    total: 0,
    currentPage: 1,
    pageSize: 10,
    isInitialized: false,
    // User registration settings
    userRegistrationEnabled: true,
    registrationSettingsInitialized: false,
    loadingRegistrationSettings: false,
    loading: false,
    creating: false,
    updating: false,
    deleting: false,
    error: null as string | null,
  },
  actions: (set, get) => {
    const loadUsers = async (page?: number, pageSize?: number) => {
      if (!hasPermissionNow(Permissions.UsersRead)) return
      try {
        const currentState = get()
        const requestPage = page || currentState.currentPage
        const requestPageSize = pageSize || currentState.pageSize
        // Skip if already initialized and loading first page without explicit page.
        if (currentState.isInitialized && currentState.loading && !page) return
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
    }
    return {
      loadUsers,
      createUser: async (data: CreateUserRequest): Promise<User | undefined> => {
        if (get().creating) return
        try {
          set({ creating: true, error: null })
          const user = await ApiClient.User.create(data)
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
      updateUser: async (id: string, data: UpdateUserRequest): Promise<User | undefined> => {
        if (get().updating) return
        try {
          set({ updating: true, error: null })
          const user = await ApiClient.User.update({ user_id: id, ...data })
          // Event handler updates state (no manual state update here).
          try {
            await emitUserUpdated(user)
          } catch (eventError) {
            console.error('Failed to emit user updated event:', eventError)
          }
          set({ updating: false })
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
        if (get().updating) return
        try {
          set({ updating: true, error: null })
          await ApiClient.User.resetPassword({ user_id: id, new_password: newPassword })
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
        if (get().updating) return
        try {
          set({ updating: true, error: null })
          await ApiClient.User.toggleActive({ user_id: id })
          const currentUser = get().users.find(u => u.id === id)
          if (currentUser) {
            const updatedUser = { ...currentUser, is_active: !currentUser.is_active }
            // Event handler updates state (no manual state update here).
            try {
              await emitUserUpdated(updatedUser)
            } catch (eventError) {
              console.error('Failed to emit user updated event:', eventError)
            }
          }
          set({ updating: false })
        } catch (error) {
          set({
            error: error instanceof Error ? error.message : 'Failed to toggle user status',
            updating: false,
          })
          throw error
        }
      },
      deleteUser: async (id: string) => {
        if (get().deleting) return
        try {
          set({ deleting: true, error: null })
          await ApiClient.User.delete({ user_id: id })
          // Event handler updates state (no manual state update here).
          try {
            await emitUserDeleted(id)
          } catch (eventError) {
            console.error('Failed to emit user deleted event:', eventError)
          }
          set({ deleting: false })
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
        if (state.registrationSettingsInitialized || state.loadingRegistrationSettings) return
        try {
          set({ loadingRegistrationSettings: true, error: null })
          // TODO: Replace with actual API call when backend endpoint exists.
          set({
            userRegistrationEnabled: true, // Default for now
            registrationSettingsInitialized: true,
            loadingRegistrationSettings: false,
          })
        } catch (error) {
          set({
            error:
              error instanceof Error ? error.message : 'Failed to load registration settings',
            loadingRegistrationSettings: false,
          })
          throw error
        }
      },
      updateUserRegistrationSettings: async (enabled: boolean) => {
        if (get().updating) return
        try {
          set({ updating: true, error: null })
          // TODO: Replace with actual API call when backend endpoint exists.
          set({ userRegistrationEnabled: enabled, updating: false })
        } catch (error) {
          set({
            error:
              error instanceof Error ? error.message : 'Failed to update registration settings',
            updating: false,
          })
          throw error
        }
      },
    }
  },
  init: ({ on, set, actions }) => {
    on('user.updated', event => {
      set(state => ({
        users: state.users.map(u => (u.id === event.data.user.id ? event.data.user : u)),
      }))
    })
    on('user.deleted', event => {
      set(state => ({
        users: state.users.filter(u => u.id !== event.data.userId),
        total: state.total - 1,
      }))
    })
    // Remote sync: loadUsers self-gates on UsersRead.
    const reload = () => void actions.loadUsers()
    on('sync:user', reload)
    on('sync:reconnect', reload)
    void actions.loadUsers()
  },
})

/** The direct-handle proxy — `import { Users }; Users.users` / `Users.loadUsers()`.
 *  Importing this file self-registers the store (so `Stores.Users` resolves too). */
export const Users = registerLazyStore(UsersStoreDef)
/** Raw zustand store (kept for the type augmentation + any raw consumer). */
export const useUsersStore = UsersStoreDef.store
