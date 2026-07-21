import { ApiClient } from '@/api-client'
import type { UpdateSessionSettingsRequest } from '@/api-client/types'
import type { SessionSettingsGet, SessionSettingsSet } from '../state'
import type { SessionSettings as SessionSettingsRow } from '@/api-client/types'

export default (set: SessionSettingsSet, _get: SessionSettingsGet) =>
  async (patch: UpdateSessionSettingsRequest): Promise<SessionSettingsRow> => {
    set(s => {
      s.saving = true
      s.error = null
    })
    try {
      const row = await ApiClient.Auth.updateSessionSettings(patch)
      set(s => {
        s.settings = row
        s.saving = false
      })
      return row
    } catch (error) {
      set(s => {
        s.error = error instanceof Error ? error.message : 'Update failed'
        s.saving = false
      })
      throw error
    }
  }
