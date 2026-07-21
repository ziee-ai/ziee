import { ApiClient } from '@/api-client'
import type { McpUserPolicySet, McpUserPolicyGet } from '../state'
import { useAuthStore } from '@/modules/auth/Auth.store'

export default (set: McpUserPolicySet, get: McpUserPolicyGet) => async () => {
  if (get().loading) return
  // Endpoint is gated on `mcp_servers::read`; short-circuit when the user
  // lacks it (leave policy: null — consumers treat that as "no restrictions
  // surfaced") so the no-403 fixture doesn't flag a missing UI gate.
  const auth = useAuthStore.getState()
  const hasMcpRead =
    auth.user?.is_admin ||
    (auth.permissions ?? []).some(
      p => p === 'mcp_servers::read' || p === '*' || p === 'mcp_servers::*',
    )
  if (!hasMcpRead) {
    set(state => {
      state.isInitialized = true
    })
    return
  }
  set(state => {
    state.loading = true
    state.error = null
  })
  try {
    const policy = await ApiClient.McpUserPolicy.get()
    set(state => {
      state.policy = policy
      state.loading = false
      state.isInitialized = true
    })
  } catch (err: unknown) {
    // Defensive — gracefully degrade on any unexpected error.
    const errorMessage = (err as any)?.message ?? String(err)
    set(state => {
      state.loading = false
      state.error = errorMessage
      state.isInitialized = true
    })
  }
}
