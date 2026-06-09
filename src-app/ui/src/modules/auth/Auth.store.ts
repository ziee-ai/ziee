import { create } from 'zustand'
import { persist, subscribeWithSelector } from 'zustand/middleware'
import { ApiClient } from '@/api-client'
import type {
  LinkAccountRequest,
  LoginRequest,
  CreateUserRequest,
  User,
} from '@/api-client/types'
import { type StoreProxy } from '@/core/stores'

export interface AutoLoginResponse {
  // Nullable: the OAuth callback path passes `null` because the
  // server is the truth (initAuth() re-fetches /me right after).
  // The store handles the null case by holding isAuthenticated=false
  // + isLoading=true until /me resolves.
  user: User | null
  access_token: string
  refresh_token: string
  expires_in?: number // Seconds until token expires (optional for backward compatibility)
}

interface AuthState {
  user?: User | null
  token?: string | null
  permissions?: string[]
  // Whether the current account has a local password (drives the
  // self-service "change password" form on the profile page). False
  // for OAuth/LDAP-only accounts. Sourced from MeResponse.has_password.
  hasPassword: boolean
  isAuthenticated: boolean
  isLoading: boolean
  isInitializing: boolean
  error?: string | null

  __init__: {
    __store__: () => void
  }
  __destroy__: () => void

  // Actions
  authenticateUser: (credentials: LoginRequest) => Promise<void>
  logoutUser: () => Promise<void>
  registerNewUser: (userData: CreateUserRequest) => Promise<void>
  linkAccount: (request: LinkAccountRequest) => Promise<void>
  clearAuthenticationError: () => void
  initAuth: () => Promise<void>
  setAuthFromAutoLogin: (response: AutoLoginResponse) => void
  // Re-fetch /me and refresh the cached user/permissions/hasPassword.
  // Called after a self-service profile edit so the sidebar widget and
  // password-section visibility stay in sync without a page reload.
  refreshCurrentUser: () => Promise<void>
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
  hasPassword: false,
  isAuthenticated: false,
  isLoading: false,
  isInitializing: true,
  error: null,
}

// Module-scope handle to the visibilitychange listener so __destroy__
// can remove it (permission-plan follow-up: re-fetch /api/auth/me when
// the tab regains focus, so stale permissions self-heal after an admin
// edits the current user's group in another tab).
let visibilityListener: (() => void) | null = null

// Guards `refreshCurrentUser` so overlapping callers (mount effect +
// post-save refresh + visibility refetch) collapse to a single in-flight
// /me request instead of racing.
let refreshInFlight: Promise<void> | null = null

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
          let loginSucceeded = false
          try {
            const response = await ApiClient.Auth.login(credentials, undefined)
            loginSucceeded = true

            // Seed the token, then COMPLETE the bootstrap by fetching /me for
            // permissions. The login/register responses don't carry
            // permissions, and the app shell gates on `permissions` +
            // `isInitializing`. Finalizing both here (not relying on a
            // separate initAuth) avoids the post-setup hang: AuthGuard's
            // initAuth() races this call, early-returns on our `isLoading`,
            // and would otherwise leave `isInitializing` stuck true forever.
            set({ token: response.access_token })
            const me = await ApiClient.Auth.me(undefined, undefined)
            set({
              user: me.user,
              permissions: me.permissions,
              isAuthenticated: true,
              isLoading: false,
              isInitializing: false,
              error: null,
            })
          } catch (error) {
            // If LOGIN itself failed (bad credentials, network, etc.),
            // clear everything — there is no valid token.
            //
            // If login SUCCEEDED but the follow-up /me throws (test
            // navigation aborts the in-flight fetch, transient backend
            // hiccup, etc.), keep the token in place. The token is
            // still valid; AuthGuard's initAuth will retry /me on the
            // next mount and either succeed (→ authenticated) or get
            // a real 401 (→ redirect to /auth). Wiping the token here
            // turned every aborted /me into a logged-out session and
            // was the cause of ~200 E2E failures per parallel run.
            const isAbort =
              error instanceof TypeError &&
              /failed to fetch|network|aborted/i.test(error.message)
            const baseError = {
              error: error instanceof Error ? error.message : 'Login failed',
              isLoading: false,
              isInitializing: false,
            }
            if (!loginSucceeded || !isAbort) {
              set({
                ...baseError,
                isAuthenticated: false,
                token: null,
                user: null,
              })
            } else {
              // Login OK, /me aborted — token is still valid, leave it.
              set(baseError)
            }
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

            // Complete the bootstrap here (token → /me for permissions →
            // isInitializing:false), same as authenticateUser — so the app
            // shell doesn't hang on the spinner after registration.
            set({ token: response.access_token })
            const me = await ApiClient.Auth.me(undefined, undefined)
            set({
              user: me.user,
              permissions: me.permissions,
              isAuthenticated: true,
              isLoading: false,
              isInitializing: false,
              error: null,
            })
          } catch (error) {
            set({
              error:
                error instanceof Error ? error.message : 'Registration failed',
              isLoading: false,
              isInitializing: false,
            })
            throw error
          }
        },

        clearAuthenticationError: () => {
          set({ error: null })
        },

        linkAccount: async (request: LinkAccountRequest) => {
          // Encapsulates the OAuth-link round-trip + re-bootstrap
          // sequence so LinkAccountPage stays presentation-only:
          //   1. POST /api/auth/link_account
          //   2. Seed the access token via setAuthFromAutoLogin
          //   3. Re-fetch /me to populate user + permissions
          const res = await ApiClient.Auth.linkAccount(request, undefined)
          get().setAuthFromAutoLogin({
            user: res.user,
            access_token: res.access_token,
            refresh_token: res.refresh_token,
          })
          await get().initAuth()
        },

        setAuthFromAutoLogin: (response: AutoLoginResponse) => {
          // The OAuth callback flow passes a null user (the server is
          // the source of truth; initAuth() re-fetches /me right
          // after). During the gap between this set() and the
          // initAuth resolve, code that reads `user.something`
          // would crash on null. Hold isAuthenticated=false until
          // we have a real user.
          //
          // CRITICAL: use `isInitializing`, NOT `isLoading`. initAuth
          // early-returns when isLoading is already true — setting it
          // here would silently skip the /me fetch and the user gets
          // bounced back to /auth by AuthGuard. AuthGuard already
          // gates its spinner on isInitializing during the bootstrap
          // path, so the UX (spinner instead of login flash) is
          // identical. (round-5 audit finding.)
          if (response.user == null) {
            set({
              user: null,
              token: response.access_token,
              isAuthenticated: false,
              isInitializing: true,
              error: null,
            })
            return
          }
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
            // Re-fetch /me when the tab regains focus, so a permissions
            // change made by an admin in another tab self-heals here on
            // the next interaction (permission-plan follow-up).
            visibilityListener = () => {
              if (document.visibilityState !== 'visible') return
              const state = get()
              if (!state.token || state.isLoading) return
              ApiClient.Auth.me(undefined, undefined)
                .then(response => {
                  set({
                    user: response.user,
                    permissions: response.permissions,
                    hasPassword: response.has_password,
                  })
                })
                .catch(err => {
                  // 401 → user's session was revoked elsewhere; let the
                  // next API call's normal error handling kick in rather
                  // than logging the user out here (which would lose
                  // any in-progress work).
                  console.warn('[Auth] visibility-refetch /me failed:', err)
                })
            }
            document.addEventListener('visibilitychange', visibilityListener)
          },
        },

        // Remove the visibilitychange listener on store destroy so
        // listener slots don't accumulate per destroy/re-init cycle.
        // (permission follow-up)
        __destroy__: () => {
          if (visibilityListener) {
            document.removeEventListener('visibilitychange', visibilityListener)
            visibilityListener = null
          }
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
                hasPassword: response.has_password,
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
            // Same logic as `authenticateUser`: a "Failed to fetch"
            // / aborted error doesn't prove the token is bad — it
            // usually means the page navigated mid-/me. Preserve
            // the token so the next mount can retry. Only wipe on
            // a real auth failure (401 surfaces here as a non-
            // TypeError with a status-bearing message).
            const isAbort =
              error instanceof TypeError &&
              /failed to fetch|network|aborted/i.test(error.message)
            const baseError = {
              error:
                error instanceof Error
                  ? error.message
                  : 'Failed to fetch user information',
              isLoading: false,
              isInitializing: false,
            }
            if (isAbort) {
              // Keep token; AuthGuard will retry initAuth on next mount.
              set(baseError)
              return
            }
            set({
              ...baseError,
              isAuthenticated: false,
              token: null,
              user: null,
            })
          }
        },

        refreshCurrentUser: async () => {
          // Collapse concurrent callers onto one in-flight /me request.
          if (refreshInFlight) return refreshInFlight
          refreshInFlight = (async () => {
            try {
              const response = await ApiClient.Auth.me(undefined, undefined)
              set({
                user: response.user,
                permissions: response.permissions,
                hasPassword: response.has_password,
              })
            } finally {
              refreshInFlight = null
            }
          })()
          return refreshInFlight
        },
      }),
      {
        name: 'auth-storage',
        partialize: state => ({ token: state.token }),
      },
    ),
  ),
)
