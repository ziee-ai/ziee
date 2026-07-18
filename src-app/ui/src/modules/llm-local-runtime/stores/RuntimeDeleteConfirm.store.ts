import type { RuntimeVersionResponse } from '@/api-client/types'
import { defineStore } from '@ziee/framework/store-kit'

export const RuntimeDeleteConfirm = defineStore('RuntimeDeleteConfirm', {
  state: { version: null as RuntimeVersionResponse | null },
  actions: set => ({
    setVersion: (version: RuntimeVersionResponse | null) => set({ version }),
  }),
})

export const useRuntimeDeleteConfirmStore = RuntimeDeleteConfirm.store
