import { defineStore, registerLazyStore } from '@ziee/framework/store-kit'
import { Permissions } from '@/api-client/permissions'
import { hasPermissionNow } from '@/core/permissions'
import { memoryAdminState, type MemoryAdminState } from './state'
import type { Actions } from './actions.gen'

const MemoryAdminDef = defineStore<MemoryAdminState, Actions>('MemoryAdmin', {
  immer: true,
  state: memoryAdminState,
  actions: import.meta.glob('./actions/*.ts'),
  init: ({ on, actions }) => {
    const reload = () => {
      if (!hasPermissionNow(Permissions.MemoryAdminRead)) return
      void actions.load()
    }
    // Cross-device sync: another admin changed memory-admin settings.
    on('sync:memory_admin_settings', reload)
    on('sync:reconnect', reload)
    // Property-init loads hit `memory::admin::read`-gated endpoints; these fire
    // whenever ANY component reads the store (incl. the chat composer's
    // MemoryStatusPill shown to every user). Gate so non-admins don't 403.
    if (hasPermissionNow(Permissions.MemoryAdminRead)) {
      void actions.load()
      void actions.loadCandidateModels()
      void actions.loadRebuildStatus()
      void actions.loadFtsRebuildStatus()
    }
  },
})

// The raw Zustand store for gallery setup that needs direct setState.
export const MemoryAdminStore = MemoryAdminDef.store

export const MemoryAdmin = registerLazyStore(MemoryAdminDef)
export const useMemoryAdminStore = MemoryAdminDef.store
