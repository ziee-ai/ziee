import { ApiClient } from '@/api-client'
import {
  type FileRagAdminSettings,
  type LlmModel,
  Permissions,
  type UpdateFileRagAdminSettingsRequest,
} from '@/api-client/types'
import { hasPermissionNow } from '@/core/permissions'
import { defineStore } from '@/core/store-kit'

/** Candidate embedding-model row for the picker. */
export type CandidateModelRow = Pick<
  LlmModel,
  'id' | 'name' | 'display_name' | 'provider_id' | 'capabilities'
>

// Tri-state `embedding_model_id` (`Option<Option<Uuid>>`) — codegen drops the
// `null` arm; widen at the store boundary so callers can clear.
export type FileRagAdminUpdatePatch = Omit<
  UpdateFileRagAdminSettingsRequest,
  'embedding_model_id' | 'reranker_model_id'
> & {
  embedding_model_id?: string | null
  reranker_model_id?: string | null
}

const toRow = (m: LlmModel): CandidateModelRow => ({
  id: m.id,
  name: m.name,
  display_name: m.display_name,
  provider_id: m.provider_id,
  capabilities: m.capabilities,
})

export const FileRagAdmin = defineStore('FileRagAdmin', {
  immer: true,
  state: {
    settings: null as FileRagAdminSettings | null,
    embeddingModels: [] as CandidateModelRow[],
    rerankerModels: [] as CandidateModelRow[],
    loading: false,
    saving: false,
    loadingModels: false,
    triggeringReembed: false,
    triggeringBackfill: false,
    error: null as string | null,
  },
  actions: set => {
    const load = async () => {
      set(s => {
        s.loading = true
        s.error = null
      })
      try {
        const row = await ApiClient.FileRagAdmin.get()
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
    const loadEmbeddingModels = async () => {
      set(s => {
        s.loadingModels = true
      })
      try {
        // Server-filtered to `text_embedding` so the picker isn't crowded by
        // chat models (same rationale as the memory admin store).
        const body = await ApiClient.LlmModel.list({
          capability: 'text_embedding',
          page: 1,
          perPage: 200,
        })
        set(s => {
          s.embeddingModels = body.models.map(toRow)
          s.loadingModels = false
        })
      } catch (error) {
        set(s => {
          s.error = error instanceof Error ? error.message : 'Failed to load models'
          s.loadingModels = false
        })
      }
    }
    const loadRerankerModels = async () => {
      try {
        const body = await ApiClient.LlmModel.list({
          capability: 'rerank',
          page: 1,
          perPage: 200,
        })
        set(s => {
          s.rerankerModels = body.models.map(toRow)
        })
      } catch {
        /* non-fatal — the reranker section shows the hub nudge when empty */
      }
    }
    return {
      load,
      loadEmbeddingModels,
      loadRerankerModels,
      triggerReembed: async (): Promise<void> => {
        set(s => {
          s.triggeringReembed = true
          s.error = null
        })
        try {
          await ApiClient.FileRagAdmin.reembed()
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
      triggerBackfill: async (): Promise<void> => {
        set(s => {
          s.triggeringBackfill = true
          s.error = null
        })
        try {
          await ApiClient.FileRagAdmin.backfill()
          set(s => {
            s.triggeringBackfill = false
          })
        } catch (error) {
          set(s => {
            s.error = error instanceof Error ? error.message : 'Trigger failed'
            s.triggeringBackfill = false
          })
          throw error
        }
      },
      update: async (patch: FileRagAdminUpdatePatch): Promise<FileRagAdminSettings> => {
        set(s => {
          s.saving = true
          s.error = null
        })
        try {
          const row = await ApiClient.FileRagAdmin.update(
            patch as UpdateFileRagAdminSettingsRequest,
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
      },
    }
  },
  // Property-init loads hit `file_rag::admin::read`-gated endpoints; skip for
  // users without the perm so non-admins don't 403.
  init: ({ on, actions }) => {
    const reload = () => {
      if (!hasPermissionNow(Permissions.FileRagAdminRead)) return
      void actions.load()
    }
    on('sync:file_rag_admin_settings', reload)
    on('sync:reconnect', reload)
    if (hasPermissionNow(Permissions.FileRagAdminRead)) {
      void actions.load()
      void actions.loadEmbeddingModels()
      void actions.loadRerankerModels()
    }
  },
})

export const useFileRagAdminStore = FileRagAdmin.store
