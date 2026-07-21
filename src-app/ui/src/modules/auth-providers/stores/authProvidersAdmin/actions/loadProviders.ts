import { ApiClient } from '@/api-client'
import { Permissions } from '@/api-client/permissions'
import { hasPermissionNow } from '@/core/permissions'
import type { AuthProvidersAdminSet } from '../state'

export default (set: AuthProvidersAdminSet) =>
  async () => {
    // Self-gate: sync:reconnect fires for every store regardless of audience,
    // so a non-admin must not refetch this admin-only list (would 403).
    if (!hasPermissionNow(Permissions.AuthProvidersRead)) {
      // Clear the initial loading state so a non-admin mount doesn't hang.
      set(s => {
        s.loading = false
      })
      return
    }
    set(s => {
      s.loading = true
      s.error = null
    })
    try {
      const res = await ApiClient.AuthProviders.list(undefined, undefined)
      set(s => {
        s.providers = res
        s.loading = false
      })
    } catch (e: any) {
      set(s => {
        s.error = e?.message ?? 'Failed to load providers'
        s.loading = false
      })
    }
  }
