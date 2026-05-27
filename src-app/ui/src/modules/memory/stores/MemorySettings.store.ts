import { create } from 'zustand'
import { subscribeWithSelector } from 'zustand/middleware'
import { immer } from 'zustand/middleware/immer'
import { ApiClient } from '@/api-client'
import type {
  UpdateUserMemorySettingsRequest,
  UserMemorySettings,
} from '@/api-client/types'
import { emitMemorySettingsUpdated } from '@/modules/memory/events'

// Widened patch type — `retention_days` + `extraction_model_id` are
// tri-state on the backend (Option<Option<T>>): absent = leave,
// null = clear, value = set. See [[MemoryAdminUpdatePatch]] doc.
export type MemorySettingsUpdatePatch = Omit<
  UpdateUserMemorySettingsRequest,
  'retention_days' | 'extraction_model_id'
> & {
  retention_days?: number | null
  extraction_model_id?: string | null
}

interface MemorySettingsStore {
  settings: UserMemorySettings | null
  loading: boolean
  saving: boolean
  error: string | null

  __init__: {
    settings: () => Promise<void>
  }

  load: () => Promise<void>
  update: (patch: MemorySettingsUpdatePatch) => Promise<UserMemorySettings>
}

const loadSettings = async (
  set: (fn: (s: MemorySettingsStore) => void) => void,
) => {
  set(s => {
    s.loading = true
    s.error = null
  })
  try {
    const row = await ApiClient.MemorySettings.get()
    set(s => {
      s.settings = row
      s.loading = false
    })
  } catch (error) {
    set(s => {
      s.error =
        error instanceof Error ? error.message : 'Failed to load settings'
      s.loading = false
    })
  }
}

export const useMemorySettingsStore = create<MemorySettingsStore>()(
  subscribeWithSelector(
    immer((set, _get) => ({
      settings: null,
      loading: false,
      saving: false,
      error: null,

      __init__: {
        settings: () => loadSettings(set),
      },

      load: () => loadSettings(set),

      update: async (patch): Promise<UserMemorySettings> => {
        set(s => {
          s.saving = true
          s.error = null
        })
        try {
          // Cast: widened patch carries `null` arms the OpenAPI
          // codegen strips. See MemoryAdmin.store comment.
          const row = await ApiClient.MemorySettings.update(
            patch as UpdateUserMemorySettingsRequest,
          )
          set(s => {
            s.settings = row
            s.saving = false
          })
          try {
            await emitMemorySettingsUpdated(row)
          } catch (eventError) {
            console.error(
              'Failed to emit memory settings updated event:',
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
