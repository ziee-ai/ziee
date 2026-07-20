import { ApiClient } from '@/api-client'
import { type SchedulerAdminSettings, type UpdateSchedulerAdminSettings } from '@/api-client/types'
import { Permissions } from '@/api-client/permissions'
import { hasPermissionNow } from '@/core/permissions'
import { defineStore } from '@ziee/framework/store-kit'

/** Deployment-wide scheduler admin settings (quota / cadence floor / retention). */
export const SchedulerAdmin = defineStore('SchedulerAdmin', {
  immer: true,
  state: {
    settings: null as SchedulerAdminSettings | null,
    loading: false,
    saving: false,
    error: null as string | null,
  },
  actions: set => {
    const loadSettings = async () => {
      if (!hasPermissionNow(Permissions.SchedulerAdminRead)) return
      set(s => {
        s.loading = true
        s.error = null
      })
      try {
        const row = await ApiClient.SchedulerAdminSettings.get()
        set(s => {
          s.settings = row
          s.loading = false
        })
      } catch (error) {
        set(s => {
          s.loading = false
          s.error =
            error instanceof Error
              ? error.message
              : 'Failed to load scheduler settings'
        })
      }
    }

    return {
      loadSettings,
      updateSettings: async (
        patch: UpdateSchedulerAdminSettings,
      ): Promise<SchedulerAdminSettings> => {
        set(s => {
          s.saving = true
          s.error = null
        })
        try {
          const row = await ApiClient.SchedulerAdminSettings.update(patch)
          set(s => {
            s.settings = row
            s.saving = false
          })
          return row
        } catch (error) {
          set(s => {
            s.saving = false
            s.error = error instanceof Error ? error.message : 'Failed to save'
          })
          throw error
        }
      },
    }
  },
  init: ({ on, actions }) => {
    const reload = () => {
      if (!hasPermissionNow(Permissions.SchedulerAdminRead)) return
      void actions.loadSettings()
    }
    on('sync:scheduler_admin_settings', reload)
    on('sync:reconnect', reload)
    reload()
  },
})

export const useSchedulerAdminStore = SchedulerAdmin.store
