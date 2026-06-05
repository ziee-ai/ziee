import { registerSync } from '@/core/sync'
import { useAuthStore } from '@/modules/auth/Auth.store'

// A permission/group-membership change on another device. Re-bootstrap
// /auth/me so this tab's permission-gated UI updates promptly. The
// server-side 60s re-check is the backstop for stream routing.
const reBootstrap = () => {
  void useAuthStore.getState().initAuth()
}

registerSync('session', {
  onEvent: reBootstrap,
  onResync: reBootstrap,
})
