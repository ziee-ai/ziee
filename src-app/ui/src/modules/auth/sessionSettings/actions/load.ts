import { ApiClient } from '@/api-client'
import type { SessionSettingsGet, SessionSettingsSet } from '../state'

export default (set: SessionSettingsSet, _get: SessionSettingsGet) =>
  async () => {
    set(s => {
      s.loading = true
      s.error = null
    })
    try {
      const row = await ApiClient.Auth.getSessionSettings()
      set(s => {
        s.settings = row
        s.loading = false
      })
    } catch (error) {
      set(s => {
        s.error =
          error instanceof Error ? error.message : 'Failed to load session settings'
        s.loading = false
      })
    }
  }
