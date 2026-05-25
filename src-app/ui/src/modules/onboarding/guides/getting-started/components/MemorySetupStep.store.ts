import { create } from 'zustand'
import { subscribeWithSelector } from 'zustand/middleware'
import { immer } from 'zustand/middleware/immer'
import { createStoreProxy } from '@/core/stores'

// Minimal embedding-capable model row for the dropdown. The full
// `LlmModel` type lives in `@/api-client/types` but the OpenAPI bundle
// only regenerates after the backend ships — until then we keep this
// local. Phase 2 will replace with the generated DTO.
interface EmbeddingCapableModel {
  id: string
  name: string
  display_name: string | null
  provider_id: string
}

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
          // GET /api/llm-models?capability=text_embedding will be
          // available after the Phase 2 list-endpoint filter lands.
          // For Phase 1 onboarding we fetch all models and filter
          // client-side; cheap because admin model lists are small.
          const res = await fetch('/api/llm-models?page=1&per_page=200', {
            credentials: 'include',
          })
          if (!res.ok) {
            throw new Error(`Failed to load models: ${res.status}`)
          }
          const body = await res.json()
          const models: EmbeddingCapableModel[] = (body.models || body || [])
            .filter((m: any) => m?.capabilities?.text_embedding === true)
            .map((m: any) => ({
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
          const body: Record<string, unknown> = { enabled: enableMemory }
          if (enableMemory && embeddingModelId) {
            body.embedding_model_id = embeddingModelId
          }
          const res = await fetch('/api/admin/memory-settings', {
            method: 'PUT',
            credentials: 'include',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify(body),
          })
          if (!res.ok) {
            throw new Error(`Failed to save: ${res.status}`)
          }
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
