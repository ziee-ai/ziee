import { ApiClient } from '@/api-client'
import type { MemoryAuditEntry } from '@/api-client/types'
import { defineStore } from '@ziee/framework/store-kit'

export const MemoryAudit = defineStore('MemoryAudit', {
  immer: true,
  state: {
    entries: [] as MemoryAuditEntry[],
    loading: false,
    limit: 100,
    error: null as string | null,
  },
  actions: (set, get) => {
    const doLoad = async (limit: number) => {
      set(s => {
        s.loading = true
        s.error = null
      })
      try {
        const rows = await ApiClient.MemoryAudit.list({ limit })
        set(s => {
          s.entries = rows
          s.loading = false
        })
      } catch (error) {
        set(s => {
          s.error = error instanceof Error ? error.message : 'Failed to load audit entries'
          s.loading = false
        })
      }
    }
    return {
      load: (limit?: number) => doLoad(limit ?? get().limit),
      setLimit: (limit: number) => {
        set(s => {
          s.limit = limit
        })
        void doLoad(limit)
      },
    }
  },
  // Was `__init__.entries` — hydrate on first access.
  init: ({ actions }) => {
    void actions.load()
  },
})

export const useMemoryAuditStore = MemoryAudit.store
