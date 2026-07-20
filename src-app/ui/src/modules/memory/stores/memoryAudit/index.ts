import { defineStore, registerLazyStore } from '@ziee/framework/store-kit'
import { memoryAuditState } from './state'

const MemoryAuditDef = defineStore('MemoryAudit', {
  immer: true,
  state: memoryAuditState,
  lazyActions: {
    load: () => import('./actions/load'),
    setLimit: () => import('./actions/setLimit'),
  },
  init: ({ actions }) => {
    void actions.load()
  },
})
export const MemoryAudit = registerLazyStore(MemoryAuditDef)
export const useMemoryAuditStore = MemoryAuditDef.store
