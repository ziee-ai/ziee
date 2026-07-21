import { defineStore, registerLazyStore } from '@ziee/framework/store-kit'
import { Permissions } from '@/api-client/permissions'
import { hasPermissionNow } from '@/core/permissions'
import { memoryAdminState, type MemoryAdminState } from './state'
import type { Actions } from './actions.gen'

const MemoryAdminDef = defineStore<MemoryAdminState, Actions>('MemoryAdmin', {
  immer: true,
  state: memoryAdminState,
  actions: import.meta.glob('./actions/*.ts'),
  init: ({ actions }) => {
    const reload = () => {
      if (!hasPermissionNow(Permissions.MemoryAdminRead)) return
      void actions.load()
    }
    // Property-init loads hit `memory::admin::read`-gated endpoints; these fire
    // whenever ANY component reads the store (incl. the chat composer's
    // MemoryStatusPill shown to every user). Self-gate so non-admins don't 403.
    reload()
    void actions.loadCandidateModels()
    void actions.loadRebuildStatus()
    void actions.loadFtsRebuildStatus()
  },
})

// The raw Zustand store for gallery setup that needs direct setState.
export const MemoryAdminStore = MemoryAdminDef.store

export const MemoryAdmin = registerLazyStore(MemoryAdminDef)
export const useMemoryAdminStore = MemoryAdminDef.store
