import { create } from 'zustand'
import { subscribeWithSelector } from 'zustand/middleware'
import { immer } from 'zustand/middleware/immer'
import { ApiClient } from '@/api-client'
import type { MemoryAuditEntry } from '@/api-client/types'

interface MemoryAuditStore {
  entries: MemoryAuditEntry[]
  loading: boolean
  limit: number
  error: string | null

  __init__: {
    entries: () => Promise<void>
  }

  load: (limit?: number) => Promise<void>
  setLimit: (limit: number) => void
}

const loadEntries = async (
  set: (fn: (s: MemoryAuditStore) => void) => void,
  limit: number,
) => {
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
      s.error =
        error instanceof Error
          ? error.message
          : 'Failed to load audit entries'
      s.loading = false
    })
  }
}

export const useMemoryAuditStore = create<MemoryAuditStore>()(
  subscribeWithSelector(
    immer((set, get) => ({
      entries: [],
      loading: false,
      limit: 100,
      error: null,

      __init__: {
        entries: () => loadEntries(set, get().limit),
      },

      load: (limit?: number) => loadEntries(set, limit ?? get().limit),

      setLimit: (limit: number) => {
        set(s => {
          s.limit = limit
        })
        loadEntries(set, limit)
      },
    })),
  ),
)
