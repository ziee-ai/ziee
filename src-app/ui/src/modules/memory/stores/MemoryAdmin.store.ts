import { create } from 'zustand'
import { subscribeWithSelector } from 'zustand/middleware'
import { immer } from 'zustand/middleware/immer'
import { ApiClient } from '@/api-client'
import type {
  LlmModel,
  MemoryAdminSettings,
  RebuildStatus,
  UpdateMemoryAdminSettingsRequest,
} from '@/api-client/types'
import { emitMemoryAdminSettingsUpdated } from '@/modules/memory/events'

export type EmbeddingCapableModelRow = Pick<
  LlmModel,
  'id' | 'name' | 'display_name' | 'provider_id'
>

// Widened patch type. Reason: the backend's `UpdateMemoryAdminSettingsRequest`
// uses `Option<Option<T>>` for `embedding_model_id` /
// `default_extraction_model_id` / the prompt fields — tri-state
// (absent = leave, null = clear, value = set). The OpenAPI schema
// reports `type: ["string", "null"]`, but the TS codegen strips
// `null` from optional fields so the generated type is `?: T`.
// Widen at the boundary so callers can pass `null` to clear.
export type MemoryAdminUpdatePatch = Omit<
  UpdateMemoryAdminSettingsRequest,
  | 'embedding_model_id'
  | 'default_extraction_model_id'
  | 'full_summary_prompt'
  | 'incremental_summary_prompt'
> & {
  embedding_model_id?: string | null
  default_extraction_model_id?: string | null
  full_summary_prompt?: string | null
  incremental_summary_prompt?: string | null
}

interface MemoryAdminStore {
  settings: MemoryAdminSettings | null
  availableModels: EmbeddingCapableModelRow[]
  rebuildStatus: RebuildStatus | null
  loading: boolean
  saving: boolean
  loadingModels: boolean
  reembeddingTrigger: boolean
  error: string | null

  __init__: {
    settings: () => Promise<void>
    availableModels: () => Promise<void>
    rebuildStatus: () => Promise<void>
  }

  load: () => Promise<void>
  loadEmbeddingCapableModels: () => Promise<void>
  loadRebuildStatus: () => Promise<void>
  triggerReembed: () => Promise<void>
  update: (patch: MemoryAdminUpdatePatch) => Promise<MemoryAdminSettings>
}

const loadAdminSettings = async (
  set: (fn: (s: MemoryAdminStore) => void) => void,
) => {
  set(s => {
    s.loading = true
    s.error = null
  })
  try {
    const row = await ApiClient.MemoryAdmin.get()
    set(s => {
      s.settings = row
      s.loading = false
    })
  } catch (error) {
    set(s => {
      s.error =
        error instanceof Error
          ? error.message
          : 'Failed to load admin settings'
      s.loading = false
    })
  }
}

const loadEmbeddingModels = async (
  set: (fn: (s: MemoryAdminStore) => void) => void,
) => {
  set(s => {
    s.loadingModels = true
  })
  try {
    const body = await ApiClient.LlmModel.list({
      capability: 'text_embedding',
      page: 1,
      perPage: 200,
    })
    const rows: EmbeddingCapableModelRow[] = body.models.map(m => ({
      id: m.id,
      name: m.name,
      display_name: m.display_name,
      provider_id: m.provider_id,
    }))
    set(s => {
      s.availableModels = rows
      s.loadingModels = false
    })
  } catch (error) {
    set(s => {
      s.error =
        error instanceof Error
          ? error.message
          : 'Failed to load embedding models'
      s.loadingModels = false
    })
  }
}

const loadRebuildStatusInternal = async (
  set: (fn: (s: MemoryAdminStore) => void) => void,
) => {
  try {
    const status = await ApiClient.MemoryAdmin.rebuildStatus()
    set(s => {
      s.rebuildStatus = status
    })
  } catch {
    // Polling failure shouldn't surface as an error toast — worst case
    // the progress card briefly shows stale data.
  }
}

export const useMemoryAdminStore = create<MemoryAdminStore>()(
  subscribeWithSelector(
    immer((set, _get) => ({
      settings: null,
      availableModels: [],
      rebuildStatus: null,
      loading: false,
      saving: false,
      loadingModels: false,
      reembeddingTrigger: false,
      error: null,

      __init__: {
        settings: () => loadAdminSettings(set),
        availableModels: () => loadEmbeddingModels(set),
        rebuildStatus: () => loadRebuildStatusInternal(set),
      },

      load: () => loadAdminSettings(set),
      loadEmbeddingCapableModels: () => loadEmbeddingModels(set),
      loadRebuildStatus: () => loadRebuildStatusInternal(set),

      triggerReembed: async (): Promise<void> => {
        set(s => {
          s.reembeddingTrigger = true
          s.error = null
        })
        try {
          await ApiClient.MemoryAdmin.reembed()
          set(s => {
            s.reembeddingTrigger = false
          })
        } catch (error) {
          set(s => {
            s.error =
              error instanceof Error ? error.message : 'Trigger failed'
            s.reembeddingTrigger = false
          })
          throw error
        }
      },

      update: async (patch): Promise<MemoryAdminSettings> => {
        set(s => {
          s.saving = true
          s.error = null
        })
        try {
          // Cast: the OpenAPI codegen widens `Option<Option<T>>` only
          // partially (loses the `null` arm). The store accepts the
          // wider `MemoryAdminUpdatePatch`; pass through verbatim —
          // JSON.stringify writes null vs absent correctly, and the
          // backend's `deserialize_nullable_field` honors both arms.
          const row = await ApiClient.MemoryAdmin.update(
            patch as UpdateMemoryAdminSettingsRequest,
          )
          set(s => {
            s.settings = row
            s.saving = false
          })
          try {
            await emitMemoryAdminSettingsUpdated(row)
          } catch (eventError) {
            console.error(
              'Failed to emit memory admin settings updated event:',
              eventError,
            )
          }
          return row
        } catch (error) {
          set(s => {
            s.error =
              error instanceof Error ? error.message : 'Update failed'
            s.saving = false
          })
          throw error
        }
      },
    })),
  ),
)
