import { ApiClient } from '@/api-client'
import type {
  SummarizationAdminGet,
  SummarizationAdminSet,
  SummarizationAdminUpdatePatch,
} from '../state'
import type { SummarizationAdminSettings, UpdateSummarizationAdminSettingsRequest } from '@/api-client/types'

export default (set: SummarizationAdminSet, _get: SummarizationAdminGet) =>
  async (
    patch: SummarizationAdminUpdatePatch,
  ): Promise<SummarizationAdminSettings> => {
    set(s => {
      s.saving = true
      s.error = null
    })
    try {
      // Cast: codegen loses the `null` arm; JSON.stringify writes null vs
      // absent correctly and the backend's deserialize_nullable_field honors both.
      const row = await ApiClient.SummarizationAdmin.update(
        patch as UpdateSummarizationAdminSettingsRequest,
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
