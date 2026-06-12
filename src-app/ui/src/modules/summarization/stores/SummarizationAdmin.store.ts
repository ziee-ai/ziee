import { create } from 'zustand'
import { subscribeWithSelector } from 'zustand/middleware'
import { immer } from 'zustand/middleware/immer'
import { Alert } from 'antd'
import { ApiClient } from '@/api-client'
import {
  type LlmModel,
  Permissions,
  type SummarizationAdminSettings,
  type UpdateSummarizationAdminSettingsRequest,
} from '@/api-client/types'
import { hasPermissionNow } from '@/core/permissions'
import { Stores } from '@/core/stores'

export type SummarizationModelRow = Pick<
  LlmModel,
  'id' | 'name' | 'display_name' | 'provider_id'
>

// Widened patch type. The backend's
// `UpdateSummarizationAdminSettingsRequest` uses `Option<Option<T>>`
// for the model id + prompt fields — tri-state (absent = leave, null
// = clear, value = set). The TS codegen strips `null` from optional
// fields so the generated type is `?: T`; widen at the boundary so
// callers can pass `null` to clear back to the compiled default.
export type SummarizationAdminUpdatePatch = Omit<
  UpdateSummarizationAdminSettingsRequest,
  | 'default_summarization_model_id'
  | 'full_summary_prompt'
  | 'incremental_summary_prompt'
> & {
  default_summarization_model_id?: string | null
  full_summary_prompt?: string | null
  incremental_summary_prompt?: string | null
}

interface SummarizationAdminStore {
  settings: SummarizationAdminSettings | null
  availableModels: SummarizationModelRow[]
  loading: boolean
  saving: boolean
  loadingModels: boolean
  error: string | null

  __init__: {
    __store__?: () => void
    settings: () => Promise<void>
    availableModels: () => Promise<void>
  }
  __destroy__?: () => void

  load: () => Promise<void>
  loadAvailableModels: () => Promise<void>
  update: (
    patch: SummarizationAdminUpdatePatch,
  ) => Promise<SummarizationAdminSettings>
}

const GROUP = 'SummarizationAdmin'

const loadAdminSettings = async (
  set: (fn: (s: SummarizationAdminStore) => void) => void,
) => {
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
        error instanceof Error
          ? error.message
          : 'Failed to load summarization settings'
      s.loading = false
    })
  }
}

const loadChatModels = async (
  set: (fn: (s: SummarizationAdminStore) => void) => void,
) => {
  set(s => {
    s.loadingModels = true
  })
  try {
    // Any text-completion model can summarize — no special capability
    // gate (unlike memory's embedding-only filter). Pull all enabled
    // chat models.
    const body = await ApiClient.LlmModel.list({
      capability: 'text_completion',
      page: 1,
      perPage: 200,
    })
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
      s.error =
        error instanceof Error ? error.message : 'Failed to load models'
      s.loadingModels = false
    })
  }
}

// Suppress the unused-import warning Biome would emit when Alert isn't
// referenced — we import it as a type-hint for downstream files that
// re-export this store. (No runtime cost; the importer can tree-shake.)
void Alert

export const useSummarizationAdminStore = create<SummarizationAdminStore>()(
  subscribeWithSelector(
    immer((set, _get) => ({
      settings: null,
      availableModels: [],
      loading: false,
      saving: false,
      loadingModels: false,
      error: null,

      __init__: {
        __store__: () => {
          const eventBus = Stores.EventBus
          // Singleton row; sync entity id is nil. Self-gate on
          // `summarization::settings::read` to skip refetches for
          // non-admins (the chat-extension pill code reads from this
          // store on every conversation switch).
          const reload = () => {
            if (!hasPermissionNow(Permissions.SummarizationSettingsRead))
              return
            void loadAdminSettings(set)
          }
          eventBus.on('sync:summarization_admin_settings', reload, GROUP)
          eventBus.on('sync:reconnect', reload, GROUP)
        },
        settings: () =>
          hasPermissionNow(Permissions.SummarizationSettingsRead)
            ? loadAdminSettings(set)
            : Promise.resolve(),
        availableModels: () =>
          hasPermissionNow(Permissions.SummarizationSettingsRead)
            ? loadChatModels(set)
            : Promise.resolve(),
      },

      __destroy__: () => {
        Stores.EventBus.removeGroupListeners(GROUP)
      },

      load: () => loadAdminSettings(set),
      loadAvailableModels: () => loadChatModels(set),

      update: async (patch): Promise<SummarizationAdminSettings> => {
        set(s => {
          s.saving = true
          s.error = null
        })
        try {
          // Cast: the codegen widens `Option<Option<T>>` only partially
          // (loses the `null` arm). The store accepts the wider
          // `SummarizationAdminUpdatePatch`; JSON.stringify writes null
          // vs absent correctly, and the backend's
          // `deserialize_nullable_field` honors both arms.
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
    })),
  ),
)
