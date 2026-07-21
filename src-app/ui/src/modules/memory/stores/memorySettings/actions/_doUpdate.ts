import { ApiClient } from '@/api-client'
import type {
  UpdateUserMemorySettingsRequest,
  UserMemorySettings,
} from '@/api-client/types'
import type { MemorySettingsGet, MemorySettingsSet } from '../state'

// Widened patch type — `retention_days` + `extraction_model_id` are
// tri-state on the backend (Option<Option<T>>): absent = leave,
// null = clear, value = set.
export type MemorySettingsUpdatePatch = Omit<
  UpdateUserMemorySettingsRequest,
  'retention_days' | 'extraction_model_id'
> & {
  retention_days?: number | null
  extraction_model_id?: string | null
}

export default (set: MemorySettingsSet, _get: MemorySettingsGet) =>
  async (patch: MemorySettingsUpdatePatch): Promise<UserMemorySettings> => {
    set(s => {
      s.saving = true
      s.error = null
    })
    try {
      // Cast: widened patch carries `null` arms the OpenAPI codegen strips.
      const row = await ApiClient.MemorySettings.update(
        patch as UpdateUserMemorySettingsRequest,
      )
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
