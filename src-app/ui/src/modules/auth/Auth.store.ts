import { create } from 'zustand'
import { persist, subscribeWithSelector } from 'zustand/middleware'
import { ApiClient } from '@/api-client'
import type {
  LoginRequest,
  CreateUserRequest,
  User,
} from '@/api-client/types'
import { Stores, type StoreProxy } from '@/core/stores'
import '@/modules/onboarding-screen/events/types'

export interface AutoLoginResponse {
  user: User
  access_token: string
  refresh_token: string
  expires_in?: number // Seconds until token expires (optional for backward compatibility)
}

interface AuthState {
  user?: User | null
  token?: string | null
  permissions?: string[]
  isAuthenticated: boolean
  isLoading: boolean
  isInitializing: boolean
  error?: string | null

  __init__: {
    __store__: () => void
  }

  // Actions
  authenticateUser: (credentials: LoginRequest) => Promise<void>
  logoutUser: () => Promise<void>
  registerNewUser: (userData: CreateUserRequest) => Promise<void>
  clearAuthenticationError: () => void
  initAuth: () => Promise<void>
  setAuthFromAutoLogin: (response: AutoLoginResponse) => void
}

// Augment the RegisteredStores interface for IntelliSense
declare module '../../core/stores' {
  interface RegisteredStores {
    Auth: StoreProxy<AuthState>
  }
}

const defaultState = {
  user: null,
  token: null,
  permissions: [],
  isAuthenticated: false,
  isLoading: false,
  isInitializing: true,
  error: null,
}

export const useAuthStore = create<AuthState>()(
  subscribeWithSelector(
    persist(
      (set, get): AuthState => ({
        ...defaultState,

        // Actions
        authenticateUser: async (credentials: LoginRequest) => {
          const state = get()
          if (state.isLoading) {
            return
          }
          set({ isLoading: true, error: null })
          try {
            const response = await ApiClient.Auth.login(credentials, undefined)

            set({
              user: response.user,
              token: response.access_token,
              isAuthenticated: true,
              isLoading: false,
              error: null,
            })
          } catch (error) {
            set({
              error: error instanceof Error ? error.message : 'Login failed',
              isLoading: false,
              isAuthenticated: false,
              token: null,
              user: null,
            })
            throw error
          }
        },

        logoutUser: async () => {
          const state = get()
          if (state.isLoading) {
            return
          }
          set({ isLoading: true, error: null })
          try {
            const { token } = get()
            if (token) {
              // Call logout API to invalidate token on server
              await ApiClient.Auth.logout(undefined, undefined)
            }

            set({
              user: null,
              token: null,
              isAuthenticated: false,
              isLoading: false,
              error: null,
            })
          } catch {
            // Even if logout fails on server, clear local state
            set({
              user: null,
              token: null,
              isAuthenticated: false,
              isLoading: false,
              error: null,
            })
          }
        },

        registerNewUser: async (userData: CreateUserRequest) => {
          const state = get()
          if (state.isLoading) {
            throw new Error('Request already in progress')
          }
          set({ isLoading: true, error: null })
          try {
            const response = await ApiClient.Auth.register(userData, undefined)

            set({
              user: response.user,
              token: response.access_token,
              isAuthenticated: true,
              isLoading: false,
              error: null,
            })
          } catch (error) {
            set({
              error:
                error instanceof Error ? error.message : 'Registration failed',
              isLoading: false,
            })
            throw error
          }
        },

        clearAuthenticationError: () => {
          set({ error: null })
        },

        setAuthFromAutoLogin: (response: AutoLoginResponse) => {
          set({
            user: response.user,
            token: response.access_token,
            isAuthenticated: true,
            isLoading: false,
            error: null,
          })
        },

        __init__: {
          __store__: () => {
            Stores.EventBus.on(
              'onboarding_screen.user_updated',
              (event) => {
                set(state => ({
                  user: state.user
                    ? {
                        ...state.user,
                        completed_onboarding_ids: event.data.user.completed_onboarding_ids,
                        completed_onboarding_step_ids: event.data.user.completed_onboarding_step_ids,
                      }
                    : state.user,
                }))
              },
              'AuthStore',
            )
          },
        },

        initAuth: async () => {
          const state = get()
          if (state.isLoading) {
            return
          }
          set({ isLoading: true, isInitializing: true, error: null })

          try {
            const token = get().token
            if (token) {
              // Fetch current user profile with permissions
              const response = await ApiClient.Auth.me(undefined, undefined)
              set({
                user: response.user,
                permissions: response.permissions,
                isAuthenticated: true,
                isLoading: false,
                isInitializing: false,
              })
            } else {
              set({
                isAuthenticated: false,
                isLoading: false,
                isInitializing: false,
              })
            }
          } catch (error) {
            set({
              error:
                error instanceof Error
                  ? error.message
                  : 'Failed to fetch user information',
              isLoading: false,
              isInitializing: false,
              isAuthenticated: false,
              token: null,
              user: null,
            })
          }
        },
      }),
      {
        name: 'auth-storage',
        partialize: state => ({ token: state.token }),
      },
    ),
  ),
)
