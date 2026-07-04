import type { RuntimeEngine } from '../types'
import { defineStore } from '@/core/store-kit'

export const RuntimeDownloadDrawer = defineStore('RuntimeDownloadDrawer', {
  state: { open: false, engine: null as RuntimeEngine | null },
  actions: set => ({
    openDrawer: (engine: RuntimeEngine) => set({ open: true, engine }),
    closeDrawer: () => set({ open: false, engine: null }),
  }),
})

export const useRuntimeDownloadDrawerStore = RuntimeDownloadDrawer.store
