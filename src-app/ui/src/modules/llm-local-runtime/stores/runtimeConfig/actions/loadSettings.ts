import { ApiClient } from '@/api-client'
import { Permissions } from '@/api-client/permissions'
import { hasPermissionNow } from '@/core/permissions'
import type { RuntimeConfigGet, RuntimeConfigSet } from '../state'

export default (set: RuntimeConfigSet, _get: RuntimeConfigGet) =>
  async () => {
    if (!hasPermissionNow(Permissions.RuntimeSettingsRead)) return
    set(s => {
      s.loadingSettings = true
      s.error = null
    })
    try {
      const settings = await ApiClient.LocalRuntime.getRuntimeSettings(undefined)
      set(s => {
        s.settings = settings
        s.loadingSettings = false
      })
    } catch (error) {
      set(s => {
        s.error =
          error instanceof Error ? error.message : 'Failed to load runtime settings'
        s.loadingSettings = false
      })
    }
  }
