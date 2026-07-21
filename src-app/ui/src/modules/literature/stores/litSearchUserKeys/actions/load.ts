import { ApiClient } from '@/api-client'
import { Permissions } from '@/api-client/permissions'
import { hasPermissionNow } from '@/core/permissions'
import type { LitSearchUserKeysGet, LitSearchUserKeysSet } from '../state'

export default (set: LitSearchUserKeysSet, _get: LitSearchUserKeysGet) =>
  async () => {
    if (!hasPermissionNow(Permissions.LitSearchUse)) return
    set(s => {
      s.loading = true
      s.error = null
    })
    try {
      const res = await ApiClient.LitSearch.listUserKeys()
      set(s => {
        s.connectors = res.connectors
        s.loading = false
      })
    } catch (error) {
      set(s => {
        s.error =
          error instanceof Error ? error.message : 'Failed to load your literature keys'
        s.loading = false
      })
    }
  }
