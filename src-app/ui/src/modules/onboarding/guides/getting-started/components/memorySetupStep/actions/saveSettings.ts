import { ApiClient } from '@/api-client'
import type { UpdateMemoryAdminSettingsRequest } from '@/api-client/types'
import type { MemorySetupStepGet, MemorySetupStepSet } from '../state'

export default (set: MemorySetupStepSet, get: MemorySetupStepGet) =>
  async (): Promise<boolean> => {
    const { enableMemory, embeddingModelId } = get()
    set(draft => {
      draft.saving = true
      draft.error = null
    })
    try {
      // ONLY include `embedding_model_id` when the admin opted in and picked a
      // model — an empty patch would clear an existing setting.
      const patch: UpdateMemoryAdminSettingsRequest = { enabled: enableMemory }
      if (enableMemory && embeddingModelId) patch.embedding_model_id = embeddingModelId
      await ApiClient.MemoryAdmin.update(patch)
      set(draft => {
        draft.saving = false
      })
      return true
    } catch (e: any) {
      set(draft => {
        draft.error = e?.message || 'Failed to save settings'
        draft.saving = false
      })
      return false
    }
  }
