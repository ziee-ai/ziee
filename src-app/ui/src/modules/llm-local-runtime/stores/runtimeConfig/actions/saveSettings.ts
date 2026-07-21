import { ApiClient } from '@/api-client'
import { type UpdateRuntimeSettingsRequest } from '@/api-client/types'
import type { RuntimeConfigGet, RuntimeConfigSet } from '../state'

export default (set: RuntimeConfigSet, _get: RuntimeConfigGet) =>
  async (req: UpdateRuntimeSettingsRequest) => {
    set(s => {
      s.savingSettings = true
      s.error = null
    })
    try {
      const settings = await ApiClient.LocalRuntime.updateRuntimeSettings(req)
      set(s => {
        s.settings = settings
        s.savingSettings = false
      })
    } catch (error) {
      set(s => {
        s.error =
          error instanceof Error ? error.message : 'Failed to save runtime settings'
        s.savingSettings = false
      })
      throw error
    }
  }
