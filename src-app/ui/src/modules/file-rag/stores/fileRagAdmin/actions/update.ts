import { ApiClient } from '@/api-client'
import type {
  FileRagAdminGet,
  FileRagAdminSet,
  FileRagAdminUpdatePatch,
} from '../state'
import type { FileRagAdminSettings, UpdateFileRagAdminSettingsRequest } from '@/api-client/types'

export default (set: FileRagAdminSet, _get: FileRagAdminGet) =>
  async (patch: FileRagAdminUpdatePatch): Promise<FileRagAdminSettings> => {
    set(s => {
      s.saving = true
      s.error = null
    })
    try {
      // embedding_model_id / reranker_model_id are widened to `string | null`
      // at the store boundary so callers can clear; the API accepts the cast.
      const row = await ApiClient.FileRagAdmin.update(
        patch as unknown as UpdateFileRagAdminSettingsRequest,
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
