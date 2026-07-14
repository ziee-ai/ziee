import { ApiClient } from '@/api-client'
import type { LlmModel, UpdateMemoryAdminSettingsRequest } from '@/api-client/types'
import { defineStore } from '@ziee/framework/store-kit'
import { createStoreProxy } from '@ziee/framework/stores'

// Picks the small subset of `LlmModel` the embedding-model dropdown needs.
type EmbeddingCapableModel = Pick<
  LlmModel,
  'id' | 'name' | 'display_name' | 'provider_id'
>

export const MemorySetupStep = defineStore('MemorySetupStep', {
  immer: true,
  state: {
    enableMemory: false,
    embeddingModelId: null as string | null,
    availableModels: [] as EmbeddingCapableModel[],
    loading: false,
    saving: false,
    error: null as string | null,
  },
  actions: (set, get) => ({
    setEnableMemory: (enabled: boolean) => {
      set(draft => {
        draft.enableMemory = enabled
      })
    },
    setEmbeddingModelId: (id: string | null) => {
      set(draft => {
        draft.embeddingModelId = id
      })
    },
    loadEmbeddingCapableModels: async () => {
      set(draft => {
        draft.loading = true
        draft.error = null
      })
      try {
        // Server-side filter `?capability=text_embedding` on the typed endpoint.
        const body = await ApiClient.LlmModel.list({
          capability: 'text_embedding',
          page: 1,
          perPage: 200,
        })
        const models: EmbeddingCapableModel[] = body.models.map(m => ({
          id: m.id,
          name: m.name,
          display_name: m.display_name,
          provider_id: m.provider_id,
        }))
        set(draft => {
          draft.availableModels = models
          draft.loading = false
        })
      } catch (e: any) {
        set(draft => {
          draft.error = e?.message || 'Failed to load embedding-capable models'
          draft.loading = false
        })
      }
    },
    saveSettings: async (): Promise<boolean> => {
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
    },
    reset: () => {
      set(draft => {
        draft.enableMemory = false
        draft.embeddingModelId = null
        draft.availableModels = []
        draft.loading = false
        draft.saving = false
        draft.error = null
      })
    },
  }),
})

export const useMemorySetupStepStore = MemorySetupStep.store
export const MemorySetupStepStoreProxy = createStoreProxy(useMemorySetupStepStore)
