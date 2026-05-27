import { create } from 'zustand'
import { subscribeWithSelector } from 'zustand/middleware'
import { immer } from 'zustand/middleware/immer'
import { createStoreProxy } from '@/core/stores'
import { ApiClient } from '@/api-client'
import type {
  LlmModel,
  UpdateMemoryAdminSettingsRequest,
} from '@/api-client/types'

// Picks the small subset of `LlmModel` the embedding-model dropdown
// needs. Keeps the store payload light and the JSX simple.
type EmbeddingCapableModel = Pick<
  LlmModel,
  'id' | 'name' | 'display_name' | 'provider_id'
>

interface MemorySetupStepStore {
  enableMemory: boolean
  embeddingModelId: string | null
  availableModels: EmbeddingCapableModel[]
  loading: boolean
  saving: boolean
  error: string | null

  setEnableMemory: (enabled: boolean) => void
  setEmbeddingModelId: (id: string | null) => void
  loadEmbeddingCapableModels: () => Promise<void>
  saveSettings: () => Promise<boolean>
  reset: () => void
}

export const useMemorySetupStepStore = create<MemorySetupStepStore>()(
  subscribeWithSelector(
    immer((set, get) => ({
      enableMemory: false,
      embeddingModelId: null,
      availableModels: [],
      loading: false,
      saving: false,
      error: null,

      setEnableMemory: (enabled) => {
        set((draft) => {
          draft.enableMemory = enabled
        })
      },

      setEmbeddingModelId: (id) => {
        set((draft) => {
          draft.embeddingModelId = id
        })
      },

      loadEmbeddingCapableModels: async () => {
        set((draft) => {
          draft.loading = true
          draft.error = null
        })
        try {
          // Server-side filter `?capability=text_embedding` (Phase 2)
          // is exposed on the typed `LlmModel.list` endpoint.
          const body = await ApiClient.LlmModel.list({
            capability: 'text_embedding',
            page: 1,
            perPage: 200,
          })
          const models: EmbeddingCapableModel[] = body.models.map((m) => ({
            id: m.id,
            name: m.name,
            display_name: m.display_name,
            provider_id: m.provider_id,
          }))
          set((draft) => {
            draft.availableModels = models
            draft.loading = false
          })
        } catch (e: any) {
          set((draft) => {
            draft.error = e?.message || 'Failed to load embedding-capable models'
            draft.loading = false
          })
        }
      },

      saveSettings: async () => {
        const { enableMemory, embeddingModelId } = get()
        set((draft) => {
          draft.saving = true
          draft.error = null
        })
        try {
          // The PUT body matches `UpdateMemoryAdminSettingsRequest`. We
          // ONLY include `embedding_model_id` when the admin opted in
          // and picked a model — sending an empty patch would clear an
          // existing setting, which is wrong during onboarding.
          const patch: UpdateMemoryAdminSettingsRequest = {
            enabled: enableMemory,
          }
          if (enableMemory && embeddingModelId) {
            patch.embedding_model_id = embeddingModelId
          }
          await ApiClient.MemoryAdmin.update(patch)
          set((draft) => {
            draft.saving = false
          })
          return true
        } catch (e: any) {
          set((draft) => {
            draft.error = e?.message || 'Failed to save settings'
            draft.saving = false
          })
          return false
        }
      },

      reset: () => {
        set((draft) => {
          draft.enableMemory = false
          draft.embeddingModelId = null
          draft.availableModels = []
          draft.loading = false
          draft.saving = false
          draft.error = null
        })
      },
    })),
  ),
)

export const MemorySetupStepStoreProxy = createStoreProxy(useMemorySetupStepStore)
