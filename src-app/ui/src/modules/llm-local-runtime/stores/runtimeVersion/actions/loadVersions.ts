import { ApiClient } from '@/api-client'
import { Permissions } from '@/api-client/permissions'
import { hasPermissionNow } from '@/core/permissions'
import type { RuntimeVersionGet, RuntimeVersionSet } from '../state'
import type { RuntimeEngine } from '../../../types'

export default (set: RuntimeVersionSet, _get: RuntimeVersionGet) =>
  async (engine?: RuntimeEngine) => {
    if (!hasPermissionNow(Permissions.RuntimeVersionRead)) return
    set(s => {
      s.loading = true
      s.error = null
    })
    try {
      const response = await ApiClient.RuntimeVersion.list({ engine })
      set(s => {
        s.versions = response.versions || []
        s.isInitialized = true
        s.loading = false
      })
    } catch (error) {
      set(s => {
        s.error = error instanceof Error ? error.message : 'Failed to load versions'
        s.loading = false
      })
    }
  }
