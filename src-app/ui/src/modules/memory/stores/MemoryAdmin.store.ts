import { ApiClient } from '@/api-client'
import {
  type FtsRebuildStatus,
  type LlmModel,
  type MemoryAdminSettings,
  Permissions,
  type RebuildStatus,
  type UpdateMemoryAdminSettingsRequest,
} from '@/api-client/types'
import { hasPermissionNow } from '@/core/permissions'
import { defineStore } from '@ziee/framework/store-kit'
import { emitMemoryAdminSettingsUpdated } from '@/modules/memory/events'

// Candidate model row for the admin form's model pickers. Carries
// `capabilities` so the form can derive the extraction list client-side.
export type CandidateModelRow = Pick<
  LlmModel,
  'id' | 'name' | 'display_name' | 'provider_id' | 'capabilities'
>

// Widened patch type. The backend uses `Option<Option<T>>` for the model id +
// prompt fields — tri-state (absent = leave, null = clear, value = set). The TS
// codegen strips `null`; widen at the boundary so callers can clear.
export type MemoryAdminUpdatePatch = Omit<
  UpdateMemoryAdminSettingsRequest,
  'embedding_model_id' | 'default_extraction_model_id'
> & {
  embedding_model_id?: string | null
  default_extraction_model_id?: string | null
}

const toRow = (m: LlmModel): CandidateModelRow => ({
  id: m.id,
  name: m.name,
  display_name: m.display_name,
  provider_id: m.provider_id,
  capabilities: m.capabilities,
})

export const MemoryAdmin = defineStore('MemoryAdmin', {
  immer: true,
  state: {
    settings: null as MemoryAdminSettings | null,
    // All models (capped page), used to derive the extraction-model list.
    availableModels: [] as CandidateModelRow[],
    // Embedding-capable models, server-filtered so the embedding picker isn't
    // truncated by unrelated chat models.
    embeddingModels: [] as CandidateModelRow[],
    rebuildStatus: null as RebuildStatus | null,
    ftsRebuildStatus: null as FtsRebuildStatus | null,
    loading: false,
    saving: false,
    loadingModels: false,
    triggeringReembed: false,
    triggeringFtsRebuild: false,
    error: null as string | null,
  },
  actions: set => {
    const load = async () => {
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
          s.error = error instanceof Error ? error.message : 'Failed to load admin settings'
          s.loading = false
        })
      }
    }
    const loadCandidateModels = async () => {
      set(s => {
        s.loadingModels = true
      })
      try {
        // Two capped fetches: embedding picker (server-filtered) + all models
        // (extraction picker keeps the non-embedders).
        const [allBody, embeddingBody] = await Promise.all([
          ApiClient.LlmModel.list({ page: 1, perPage: 200 }),
          ApiClient.LlmModel.list({ capability: 'text_embedding', page: 1, perPage: 200 }),
        ])
        set(s => {
          // Extraction picker = all models MINUS embedders ("not an embedder"
          // rather than "is chat", so a chat model with no capability flag
          // still appears).
          s.availableModels = allBody.models
            .map(toRow)
            .filter(m => !m.capabilities?.text_embedding)
          s.embeddingModels = embeddingBody.models.map(toRow)
          s.loadingModels = false
        })
      } catch (error) {
        set(s => {
          s.error = error instanceof Error ? error.message : 'Failed to load models'
          s.loadingModels = false
        })
      }
    }
    const loadRebuildStatus = async () => {
      try {
        const status = await ApiClient.MemoryAdmin.rebuildStatus()
        set(s => {
          s.rebuildStatus = status
        })
      } catch {
        // Polling failure shouldn't surface as an error toast.
      }
    }
    const loadFtsRebuildStatus = async () => {
      try {
        const status = await ApiClient.MemoryAdmin.ftsRebuildStatus()
        set(s => {
          s.ftsRebuildStatus = status
        })
      } catch {
        // Same rationale as loadRebuildStatus.
      }
    }
    return {
      load,
      loadCandidateModels,
      loadRebuildStatus,
      loadFtsRebuildStatus,
      triggerReembed: async (): Promise<void> => {
        set(s => {
          s.triggeringReembed = true
          s.error = null
        })
        try {
          await ApiClient.MemoryAdmin.reembed()
          set(s => {
            s.triggeringReembed = false
          })
        } catch (error) {
          set(s => {
            s.error = error instanceof Error ? error.message : 'Trigger failed'
            s.triggeringReembed = false
          })
          throw error
        }
      },
      triggerFtsRebuild: async (dictionary: string): Promise<void> => {
        set(s => {
          s.triggeringFtsRebuild = true
          s.error = null
        })
        try {
          await ApiClient.MemoryAdmin.ftsRebuild({ dictionary })
          set(s => {
            s.triggeringFtsRebuild = false
          })
        } catch (error) {
          set(s => {
            s.error = error instanceof Error ? error.message : 'Trigger failed'
            s.triggeringFtsRebuild = false
          })
          throw error
        }
      },
      update: async (patch: MemoryAdminUpdatePatch): Promise<MemoryAdminSettings> => {
        set(s => {
          s.saving = true
          s.error = null
        })
        try {
          // Cast: codegen loses the `null` arm; JSON.stringify writes null vs
          // absent correctly and the backend's deserialize_nullable_field honors both.
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
            console.error('Failed to emit memory admin settings updated event:', eventError)
          }
          return row
        } catch (error) {
          set(s => {
            s.error = error instanceof Error ? error.message : 'Update failed'
            s.saving = false
          })
          throw error
        }
      },
    }
  },
  // Property-init loads hit `memory::admin::read`-gated endpoints; these fire
  // whenever ANY component reads the store (incl. the chat composer's
  // MemoryStatusPill shown to every user). Self-gate so non-admins don't 403.
  init: ({ on, actions }) => {
    const reload = () => {
      if (!hasPermissionNow(Permissions.MemoryAdminRead)) return
      void actions.load()
    }
    on('sync:memory_admin_settings', reload)
    on('sync:reconnect', reload)
    if (hasPermissionNow(Permissions.MemoryAdminRead)) {
      void actions.load()
      void actions.loadCandidateModels()
      void actions.loadRebuildStatus()
      void actions.loadFtsRebuildStatus()
    }
  },
})

export const useMemoryAdminStore = MemoryAdmin.store
