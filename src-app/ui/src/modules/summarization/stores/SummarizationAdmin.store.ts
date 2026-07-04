import { ApiClient } from '@/api-client'
import {
  type LlmModel,
  Permissions,
  type SummarizationAdminSettings,
  type UpdateSummarizationAdminSettingsRequest,
} from '@/api-client/types'
import { hasPermissionNow } from '@/core/permissions'
import { defineStore } from '@/core/store-kit'

export type SummarizationModelRow = Pick<
  LlmModel,
  'id' | 'name' | 'display_name' | 'provider_id'
>

// Widened patch type. The backend uses `Option<Option<T>>` for the model id +
// prompt fields — tri-state (absent = leave, null = clear, value = set). The TS
// codegen strips `null`; widen at the boundary so callers can clear.
export type SummarizationAdminUpdatePatch = Omit<
  UpdateSummarizationAdminSettingsRequest,
  'default_summarization_model_id' | 'full_summary_prompt' | 'incremental_summary_prompt'
> & {
  default_summarization_model_id?: string | null
  full_summary_prompt?: string | null
  incremental_summary_prompt?: string | null
}

export const SummarizationAdmin = defineStore('SummarizationAdmin', {
  immer: true,
  state: {
    settings: null as SummarizationAdminSettings | null,
    availableModels: [] as SummarizationModelRow[],
    loading: false,
    saving: false,
    loadingModels: false,
    error: null as string | null,
  },
  actions: set => {
    const load = async () => {
      set(s => {
        s.loading = true
        s.error = null
      })
      try {
        const row = await ApiClient.SummarizationAdmin.get()
        set(s => {
          s.settings = row
          s.loading = false
        })
      } catch (error) {
        set(s => {
          s.error =
            error instanceof Error ? error.message : 'Failed to load summarization settings'
          s.loading = false
        })
      }
    }
    const loadAvailableModels = async () => {
      // The picker lists `/api/llm-models` (requires LlmModelsRead). A user who
      // only holds summarization::settings::read can VIEW but must not trigger
      // that fetch (would 403 — no-403 self-gating rule). Skip quietly.
      if (!hasPermissionNow(Permissions.LlmModelsRead)) {
        set(s => {
          s.availableModels = []
          s.loadingModels = false
        })
        return
      }
      set(s => {
        s.loadingModels = true
      })
      try {
        // Any chat-capable model can summarize — pass `chat` (an earlier draft
        // passed `text_completion`, which the backend rejects with 400).
        const body = await ApiClient.LlmModel.list({ capability: 'chat', page: 1, perPage: 200 })
        const rows: SummarizationModelRow[] = body.models.map(m => ({
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
          s.error = error instanceof Error ? error.message : 'Failed to load models'
          s.loadingModels = false
        })
      }
    }
    return {
      load,
      loadAvailableModels,
      update: async (
        patch: SummarizationAdminUpdatePatch,
      ): Promise<SummarizationAdminSettings> => {
        set(s => {
          s.saving = true
          s.error = null
        })
        try {
          // Cast: codegen loses the `null` arm; JSON.stringify writes null vs
          // absent correctly and the backend's deserialize_nullable_field honors both.
          const row = await ApiClient.SummarizationAdmin.update(
            patch as UpdateSummarizationAdminSettingsRequest,
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
  init: ({ on, actions }) => {
    // Singleton row; sync entity id is nil. Self-gate on
    // summarization::settings::read to skip refetches for non-admins (the
    // chat-extension pill reads from this store on every conversation switch).
    const reload = () => {
      if (!hasPermissionNow(Permissions.SummarizationSettingsRead)) return
      void actions.load()
    }
    on('sync:summarization_admin_settings', reload)
    on('sync:reconnect', reload)
    if (hasPermissionNow(Permissions.SummarizationSettingsRead)) {
      void actions.load()
      void actions.loadAvailableModels()
    }
  },
})

export const useSummarizationAdminStore = SummarizationAdmin.store
