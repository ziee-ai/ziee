import { ApiClient } from '@/api-client'
import type { SchedulerAdminSettings, UpdateSchedulerAdminSettings } from '@/api-client/types'
import type { SchedulerAdminGet, SchedulerAdminSet } from '../state'

/** Save-patch variant — updates the API and returns the row. */
export function updateSettingsFn(set: SchedulerAdminSet, _get: SchedulerAdminGet) {
  return async (patch: UpdateSchedulerAdminSettings): Promise<SchedulerAdminSettings> => {
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
  }
}

/** Load variant — fetches current settings. */
export function loadSettingsFn(set: SchedulerAdminSet, _get: SchedulerAdminGet) {
  return async (): Promise<void> => {
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
        s.error = error instanceof Error ? error.message : 'Failed to load scheduler settings'
      })
    }
  }
}
