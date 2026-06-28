import { create } from 'zustand'
import { subscribeWithSelector } from 'zustand/middleware'
import { immer } from 'zustand/middleware/immer'
import { ApiClient } from '@/api-client'
import {
  type FileRagAdminSettings,
  type LlmModel,
  Permissions,
  type UpdateFileRagAdminSettingsRequest,
} from '@/api-client/types'
import { hasPermissionNow } from '@/core/permissions'
import { Stores } from '@/core/stores'

/** Candidate embedding-model row for the picker. */
export type CandidateModelRow = Pick<
  LlmModel,
  'id' | 'name' | 'display_name' | 'provider_id' | 'capabilities'
>

// The backend's `embedding_model_id` is tri-state (`Option<Option<Uuid>>`:
// absent = leave, null = clear, value = set), but the TS codegen drops the
// `null` arm. Widen at the store boundary so callers can pass `null` to clear.
export type FileRagAdminUpdatePatch = Omit<
  UpdateFileRagAdminSettingsRequest,
  'embedding_model_id'
> & {
  embedding_model_id?: string | null
}

interface FileRagAdminStore {
  settings: FileRagAdminSettings | null
  embeddingModels: CandidateModelRow[]
  loading: boolean
  saving: boolean
  loadingModels: boolean
  triggeringReembed: boolean
  triggeringBackfill: boolean
  error: string | null

  __init__: {
    __store__?: () => void
    settings: () => Promise<void>
    embeddingModels: () => Promise<void>
  }
  __destroy__?: () => void

  load: () => Promise<void>
  loadEmbeddingModels: () => Promise<void>
  triggerReembed: () => Promise<void>
  triggerBackfill: () => Promise<void>
  update: (patch: FileRagAdminUpdatePatch) => Promise<FileRagAdminSettings>
}

const loadAdminSettings = async (
  set: (fn: (s: FileRagAdminStore) => void) => void,
) => {
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
      s.error =
        error instanceof Error ? error.message : 'Failed to load admin settings'
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

const loadEmbeddingModels = async (
  set: (fn: (s: FileRagAdminStore) => void) => void,
) => {
  set(s => {
    s.loadingModels = true
  })
  try {
    // Server-filtered to `text_embedding` so the picker is never crowded out
    // by chat models (same rationale as the memory admin store).
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

export const useFileRagAdminStore = create<FileRagAdminStore>()(
  subscribeWithSelector(
    immer((set, _get) => ({
      settings: null,
      embeddingModels: [],
      loading: false,
      saving: false,
      loadingModels: false,
      triggeringReembed: false,
      triggeringBackfill: false,
      error: null,

      // Property-init loads hit `file_rag::admin::read`-gated endpoints; skip
      // the call for users without the permission so non-admins don't 403.
      __init__: {
        __store__: () => {
          const eventBus = Stores.EventBus
          const GROUP = 'FileRagAdmin'
          const reload = () => {
            if (!hasPermissionNow(Permissions.FileRagAdminRead)) return
            void loadAdminSettings(set)
          }
          eventBus.on('sync:file_rag_admin_settings', reload, GROUP)
          eventBus.on('sync:reconnect', reload, GROUP)
        },
        settings: () =>
          hasPermissionNow(Permissions.FileRagAdminRead)
            ? loadAdminSettings(set)
            : Promise.resolve(),
        embeddingModels: () =>
          hasPermissionNow(Permissions.FileRagAdminRead)
            ? loadEmbeddingModels(set)
            : Promise.resolve(),
      },

      __destroy__: () => {
        Stores.EventBus.removeGroupListeners('FileRagAdmin')
      },

      load: () => loadAdminSettings(set),
      loadEmbeddingModels: () => loadEmbeddingModels(set),

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

      update: async (patch): Promise<FileRagAdminSettings> => {
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
    })),
  ),
)
