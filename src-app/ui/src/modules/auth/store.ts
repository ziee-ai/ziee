import { create } from 'zustand'
import { persist, subscribeWithSelector } from 'zustand/middleware'
import { ApiClient } from '../../api-client'
import type {
  LoginRequest,
  CreateUserRequest,
  User,
} from '../../api-client/types'
import type { StoreProxy } from '@/core/stores'

interface AuthState {
  user?: User | null
  token?: string | null
  permissions?: string[]
  isAuthenticated: boolean
  isLoading: boolean
  error?: string | null
}

// Augment the RegisteredStores interface for IntelliSense
declare module '../../core/stores' {
  interface RegisteredStores {
    Auth: StoreProxy<AuthState>
  }
}

const defaultState: AuthState = {
  user: null,
  token: null,
  permissions: [],
  isAuthenticated: false,
  isLoading: false,
  error: null,
}

export const useAuthStore = create<AuthState>()(
  subscribeWithSelector(
    persist((): AuthState => defaultState, {
      name: 'auth-storage',
      partialize: state => ({ token: state.token }),
    }),
  ),
)

// Auth actions
export const authenticateUser = async (
  credentials: LoginRequest,
): Promise<void> => {
  const state = useAuthStore.getState()
  if (state.isLoading) {
    return
  }
  useAuthStore.setState({ isLoading: true, error: null })
  try {
    const response = await ApiClient.Auth.login(credentials, undefined)

    useAuthStore.setState({
      user: response.user,
      token: response.access_token,
      isAuthenticated: true,
      isLoading: false,
      error: null,
    })
  } catch (error) {
    useAuthStore.setState({
      error: error instanceof Error ? error.message : 'Login failed',
      isLoading: false,
      isAuthenticated: false,
      token: null,
      user: null,
    })
    throw error
  }
}

export const logoutUser = async (): Promise<void> => {
  const state = useAuthStore.getState()
  if (state.isLoading) {
    return
  }
  useAuthStore.setState({ isLoading: true, error: null })
  try {
    const { token } = useAuthStore.getState()
    if (token) {
      // Call logout API to invalidate token on server
      await ApiClient.Auth.logout(undefined, undefined)
    }

    useAuthStore.setState({
      user: null,
      token: null,
      isAuthenticated: false,
      isLoading: false,
      error: null,
    })
  } catch {
    // Even if logout fails on server, clear local state
    useAuthStore.setState({
      user: null,
      token: null,
      isAuthenticated: false,
      isLoading: false,
      error: null,
    })
  }
}

export const registerNewUser = async (
  userData: CreateUserRequest,
): Promise<void> => {
  const state = useAuthStore.getState()
  if (state.isLoading) {
    return
  }
  useAuthStore.setState({ isLoading: true, error: null })
  try {
    const response = await ApiClient.Auth.register(userData, undefined)

    useAuthStore.setState({
      user: response.user,
      token: response.access_token,
      isAuthenticated: true,
      isLoading: false,
      error: null,
    })
  } catch (error) {
    useAuthStore.setState({
      error: error instanceof Error ? error.message : 'Registration failed',
      isLoading: false,
    })
    throw error
  }
}

export const clearAuthenticationError = (): void => {
  useAuthStore.setState({ error: null })
}

export const initAuth = async (): Promise<void> => {
  const state = useAuthStore.getState()
  if (state.isLoading) {
    return
  }
  useAuthStore.setState({ isLoading: true, error: null })

  try {
    const token = useAuthStore.getState().token
    if (token) {
      // Fetch current user profile with permissions
      const response = await ApiClient.Auth.me(undefined, undefined)
      useAuthStore.setState({
        user: response.user,
        permissions: response.permissions,
        isAuthenticated: true,
        isLoading: false,
      })
    } else {
      useAuthStore.setState({
        isAuthenticated: false,
        isLoading: false,
      })
    }
  } catch (error) {
    useAuthStore.setState({
      error:
        error instanceof Error
          ? error.message
          : 'Failed to fetch user information',
      isLoading: false,
      isAuthenticated: false,
      token: null,
      user: null,
    })
  }
}

