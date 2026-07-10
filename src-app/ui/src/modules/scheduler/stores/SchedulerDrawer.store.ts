import type { ScheduledTask } from '@/api-client/types'
import { defineStore } from '@/core/store-kit'

/** Open/edit state for the scheduled-task create/edit drawer. */
export const SchedulerDrawer = defineStore('SchedulerDrawer', {
  immer: true,
  state: {
    open: false,
    editing: null as ScheduledTask | null,
    loading: false,
  },
  actions: set => ({
    openCreate: () =>
      set(s => {
        s.open = true
        s.editing = null
      }),
    openEdit: (task: ScheduledTask) =>
      set(s => {
        s.open = true
        s.editing = task
      }),
    close: () =>
      set(s => {
        s.open = false
        s.editing = null
        s.loading = false
      }),
    setLoading: (loading: boolean) =>
      set(s => {
        s.loading = loading
      }),
  }),
})

export const useSchedulerDrawerStore = SchedulerDrawer.store
