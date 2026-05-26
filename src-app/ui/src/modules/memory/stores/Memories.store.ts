import { create } from 'zustand'
import { subscribeWithSelector } from 'zustand/middleware'
import { immer } from 'zustand/middleware/immer'
import { createStoreProxy } from '@/core/stores'

export interface UserMemoryRow {
  id: string
  user_id: string
  content: string
  embedding_model: string | null
  source: 'extraction' | 'mcp_tool' | 'manual'
  source_message_id: string | null
  importance: number
  confidence: number
  kind: 'preference' | 'fact' | 'goal' | 'relationship' | 'other'
  metadata: unknown
  created_at: string
  updated_at: string
  last_recalled_at: string | null
  recall_count: number
}

interface MemoriesStore {
  memories: UserMemoryRow[]
  loading: boolean
  saving: boolean
  error: string | null
  searchQuery: string
  kindFilter: string | null
  sourceFilter: string | null

  load: () => Promise<void>
  create: (
    content: string,
    importance?: number,
    kind?: string,
  ) => Promise<UserMemoryRow | null>
  update: (
    id: string,
    patch: { content?: string; importance?: number; kind?: string },
  ) => Promise<UserMemoryRow | null>
  remove: (id: string) => Promise<boolean>
  removeAll: () => Promise<number>
  setSearchQuery: (q: string) => void
  setKindFilter: (k: string | null) => void
  setSourceFilter: (s: string | null) => void
  reset: () => void
}

export const useMemoriesStore = create<MemoriesStore>()(
  subscribeWithSelector(
    immer((set, _get) => ({
      memories: [],
      loading: false,
      saving: false,
      error: null,
      searchQuery: '',
      kindFilter: null,
      sourceFilter: null,

      load: async () => {
        set((d) => {
          d.loading = true
          d.error = null
        })
        try {
          const res = await fetch('/api/memories?limit=200', {
            credentials: 'include',
          })
          if (!res.ok) throw new Error(`Failed to load memories: ${res.status}`)
          const rows: UserMemoryRow[] = await res.json()
          set((d) => {
            d.memories = rows
            d.loading = false
          })
        } catch (e: any) {
          set((d) => {
            d.error = e?.message ?? 'Failed to load memories'
            d.loading = false
          })
        }
      },

      create: async (content, importance, kind) => {
        set((d) => {
          d.saving = true
          d.error = null
        })
        try {
          const res = await fetch('/api/memories', {
            method: 'POST',
            credentials: 'include',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({
              content,
              importance: importance ?? 50,
              kind: kind ?? 'fact',
              metadata: {},
            }),
          })
          if (!res.ok) throw new Error(`Create failed: ${res.status}`)
          const row: UserMemoryRow = await res.json()
          set((d) => {
            d.memories.unshift(row)
            d.saving = false
          })
          return row
        } catch (e: any) {
          set((d) => {
            d.error = e?.message ?? 'Create failed'
            d.saving = false
          })
          return null
        }
      },

      update: async (id, patch) => {
        set((d) => {
          d.saving = true
          d.error = null
        })
        try {
          const res = await fetch(`/api/memories/${id}`, {
            method: 'PATCH',
            credentials: 'include',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify(patch),
          })
          if (!res.ok) throw new Error(`Update failed: ${res.status}`)
          const row: UserMemoryRow = await res.json()
          set((d) => {
            const idx = d.memories.findIndex((m: UserMemoryRow) => m.id === id)
            if (idx >= 0) d.memories[idx] = row
            d.saving = false
          })
          return row
        } catch (e: any) {
          set((d) => {
            d.error = e?.message ?? 'Update failed'
            d.saving = false
          })
          return null
        }
      },

      remove: async (id) => {
        try {
          const res = await fetch(`/api/memories/${id}`, {
            method: 'DELETE',
            credentials: 'include',
          })
          if (!res.ok && res.status !== 204) {
            throw new Error(`Delete failed: ${res.status}`)
          }
          set((d) => {
            d.memories = d.memories.filter((m: UserMemoryRow) => m.id !== id)
          })
          return true
        } catch (e: any) {
          set((d) => {
            d.error = e?.message ?? 'Delete failed'
          })
          return false
        }
      },

      removeAll: async () => {
        try {
          const res = await fetch('/api/memories/all', {
            method: 'DELETE',
            credentials: 'include',
          })
          if (!res.ok) throw new Error(`Delete-all failed: ${res.status}`)
          const body: { deleted: number } = await res.json()
          set((d) => {
            d.memories = []
          })
          return body.deleted
        } catch (e: any) {
          set((d) => {
            d.error = e?.message ?? 'Delete-all failed'
          })
          return 0
        }
      },

      setSearchQuery: (q) => set((d) => { d.searchQuery = q }),
      setKindFilter: (k) => set((d) => { d.kindFilter = k }),
      setSourceFilter: (s) => set((d) => { d.sourceFilter = s }),

      reset: () =>
        set((d) => {
          d.memories = []
          d.loading = false
          d.saving = false
          d.error = null
          d.searchQuery = ''
          d.kindFilter = null
          d.sourceFilter = null
        }),
    })),
  ),
)

export const MemoriesStoreProxy = createStoreProxy(useMemoriesStore)
