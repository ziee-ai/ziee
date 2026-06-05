import { ApiClient } from '@/api-client'
import { registerSync } from '@/core/sync'
import { useAuthStore } from '@/modules/auth/Auth.store'

// A permission/group-membership/profile change on another device. Quietly
// re-fetch /auth/me and patch user + permissions so this tab's
// permission-gated UI updates. Deliberately does NOT call `initAuth()` —
// that sets `isInitializing` which blanks the whole app to a fullscreen
// spinner. Mirrors the auth store's visibilitychange refetch.
const refreshMe = () => {
  const { token, isLoading } = useAuthStore.getState()
  if (!token || isLoading) return
  void ApiClient.Auth.me(undefined, undefined)
    .then(response => {
      useAuthStore.setState({
        user: response.user,
        permissions: response.permissions,
      })
    })
    .catch(err => {
      // 401 → session revoked elsewhere; let the next API call's normal
      // error handling log the user out rather than yanking them here.
      console.warn('[sync] session refresh /me failed:', err)
    })
}

registerSync('session', {
  onEvent: refreshMe,
  onResync: refreshMe,
})

// The user's own profile changed (e.g. an admin edited their account).
registerSync('profile', {
  onEvent: refreshMe,
  onResync: refreshMe,
})
