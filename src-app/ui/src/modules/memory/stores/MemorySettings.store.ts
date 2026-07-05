import { ApiClient } from '@/api-client'
import type {
  UpdateUserMemorySettingsRequest,
  UserMemorySettings,
} from '@/api-client/types'
import { Permissions } from '@/api-client/types'
import { hasPermissionNow } from '@/core/permissions'
import { defineStore } from '@/core/store-kit'
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

export const MemorySettings = defineStore('MemorySettings', {
  immer: true,
  state: {
    settings: null as UserMemorySettings | null,
    loading: false,
    saving: false,
    error: null as string | null,
  },
  actions: set => {
    const load = async () => {
      // `sync:reconnect` fires for every store regardless of audience; skip the
      // refetch for users without `memory::read` (the endpoint would 403).
      if (!hasPermissionNow(Permissions.MemoryRead)) return
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
          s.error = error instanceof Error ? error.message : 'Failed to load settings'
          s.loading = false
        })
      }
    }
    return {
      load,
      update: async (patch: MemorySettingsUpdatePatch): Promise<UserMemorySettings> => {
        set(s => {
          s.saving = true
          s.error = null
        })
        try {
          // Cast: widened patch carries `null` arms the OpenAPI codegen strips.
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
            console.error('Failed to emit memory settings updated event:', eventError)
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
  init: ({ on, actions }) => {
    // Per-user singleton — refetch it. `load()` is permission-gated internally.
    const reload = () => void actions.load()
    on('sync:memory_settings', reload)
    on('sync:reconnect', reload)
    void actions.load()
  },
})

export const useMemorySettingsStore = MemorySettings.store
