import { create } from 'zustand'
import { subscribeWithSelector } from 'zustand/middleware'
import { immer } from 'zustand/middleware/immer'
import { ApiClient } from '@/api-client'
import {
  type LlmModel,
  type MemoryAdminSettings,
  type RebuildStatus,
  type UpdateMemoryAdminSettingsRequest,
  Permissions,
} from '@/api-client/types'
import { hasPermissionNow } from '@/core/permissions'
import { emitMemoryAdminSettingsUpdated } from '@/modules/memory/events'

// Candidate model row for the admin form's two model pickers. Carries
// `capabilities` so the form can derive the extraction list (non-embedding
// models) client-side.
export type CandidateModelRow = Pick<
  LlmModel,
  'id' | 'name' | 'display_name' | 'provider_id' | 'capabilities'
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
  // All models (capped page), used to derive the extraction-model list.
  availableModels: CandidateModelRow[]
  // Embedding-capable models, fetched server-side (`capability=text_embedding`)
  // so the embedding picker is never truncated by unrelated chat models
  // crowding out a late-added embedder in the capped page. Populated by
  // the same `loadCandidateModels` call as `availableModels`.
  embeddingModels: CandidateModelRow[]
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
  loadCandidateModels: () => Promise<void>
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

const toRow = (m: LlmModel): CandidateModelRow => ({
  id: m.id,
  name: m.name,
  display_name: m.display_name,
  provider_id: m.provider_id,
  capabilities: m.capabilities,
})

const loadCandidateModels = async (
  set: (fn: (s: MemoryAdminStore) => void) => void,
) => {
  set(s => {
    s.loadingModels = true
  })
  try {
    // Two fetches, both capped at the same page size:
    //  - embedding picker: server-filtered `capability=text_embedding`
    //    so a late-added embedder is never crowded out of the page by
    //    unrelated chat models (the original single-list bug also caused
    //    the extraction dropdown to show ONLY embedders).
    //  - extraction picker: ALL models; the form keeps the non-embedding
    //    ones (using "not an embedder" rather than "is chat" so manually
    //    added chat models without a capability flag still appear).
    const [allBody, embeddingBody] = await Promise.all([
      ApiClient.LlmModel.list({ page: 1, perPage: 200 }),
      ApiClient.LlmModel.list({
        capability: 'text_embedding',
        page: 1,
        perPage: 200,
      }),
    ])
    set(s => {
      s.availableModels = allBody.models.map(toRow)
      s.embeddingModels = embeddingBody.models.map(toRow)
      s.loadingModels = false
    })
  } catch (error) {
    set(s => {
      s.error =
        error instanceof Error ? error.message : 'Failed to load models'
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
      embeddingModels: [],
      rebuildStatus: null,
      loading: false,
      saving: false,
      loadingModels: false,
      reembeddingTrigger: false,
      error: null,

      // Property-init loads hit `memory::admin::read`-gated endpoints.
      // These fire whenever ANY component reads the field — including the
      // chat composer's MemoryStatusPill (shown to every user). Skip the
      // call for users without the permission so non-admins don't generate
      // 403s on `/api/memory/admin-settings` (the explicit `load*` actions
      // below stay ungated; they're only called from the admin-gated page).
      __init__: {
        settings: () =>
          hasPermissionNow(Permissions.MemoryAdminRead)
            ? loadAdminSettings(set)
            : Promise.resolve(),
        availableModels: () =>
          hasPermissionNow(Permissions.MemoryAdminRead)
            ? loadCandidateModels(set)
            : Promise.resolve(),
        rebuildStatus: () =>
          hasPermissionNow(Permissions.MemoryAdminRead)
            ? loadRebuildStatusInternal(set)
            : Promise.resolve(),
      },

      load: () => loadAdminSettings(set),
      loadCandidateModels: () => loadCandidateModels(set),
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
