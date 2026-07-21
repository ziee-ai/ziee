import { ApiClient } from '@/api-client'
import type { UpdateLitSearchSettingsRequest, LitSearchSettings } from '@/api-client/types'
import type { LitSearchAdminGet, LitSearchAdminSet } from '../state'

export default (set: LitSearchAdminSet, _get: LitSearchAdminGet) =>
  async (
    patch: UpdateLitSearchSettingsRequest,
  ): Promise<LitSearchSettings> => {
    set(s => {
      s.savingSettings = true
      s.error = null
    })
    try {
      const row = await ApiClient.LitSearch.updateSettings(patch)
      set(s => {
        s.settings = row
        s.savingSettings = false
      })
      return row
    } catch (error) {
      set(s => {
        s.error = error instanceof Error ? error.message : 'Update failed'
        s.savingSettings = false
      })
      throw error
    }
  }
