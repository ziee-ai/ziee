import { defineStore, registerLazyStore } from '@ziee/framework/store-kit'
import { memoryAuditState, type MemoryAuditState } from './state'
import type { Actions } from './actions.gen'

const MemoryAuditDef = defineStore<MemoryAuditState, Actions>('MemoryAudit', {
  immer: true,
  state: memoryAuditState,
  actions: import.meta.glob('./actions/*.ts'),
  init: ({ actions }) => {
    void actions.load()
  },
})
export const MemoryAudit = registerLazyStore(MemoryAuditDef)
export const useMemoryAuditStore = MemoryAuditDef.store
