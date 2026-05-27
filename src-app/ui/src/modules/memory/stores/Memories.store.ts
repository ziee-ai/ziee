import { create } from 'zustand'
import { subscribeWithSelector } from 'zustand/middleware'
import { immer } from 'zustand/middleware/immer'
import { ApiClient } from '@/api-client'
import type {
  CreateMemoryRequest,
  DeleteAllResponse,
  UpdateMemoryRequest,
  UserMemory,
} from '@/api-client/types'
import {
  emitMemoryAllCleared,
  emitMemoryCreated,
  emitMemoryDeleted,
  emitMemoryUpdated,
} from '@/modules/memory/events'

interface MemoriesStore {
  memories: UserMemory[]
  loading: boolean
  saving: boolean
  error: string | null
  searchQuery: string
  kindFilter: string | null
  sourceFilter: string | null

  __init__: {
    memories: () => Promise<void>
  }

  load: () => Promise<void>
  create: (
    content: string,
    importance?: number,
    kind?: string,
  ) => Promise<UserMemory>
  update: (
    id: string,
    patch: Omit<UpdateMemoryRequest, never>,
  ) => Promise<UserMemory>
  remove: (id: string) => Promise<void>
  removeAll: () => Promise<number>
  setSearchQuery: (q: string) => void
  setKindFilter: (k: string | null) => void
  setSourceFilter: (s: string | null) => void
  reset: () => void
}

const loadMemories = async (
  set: (fn: (s: MemoriesStore) => void) => void,
) => {
  set(s => {
    s.loading = true
    s.error = null
  })
  try {
    const rows = await ApiClient.Memory.list({ limit: 200 })
    set(s => {
      s.memories = rows
      s.loading = false
    })
  } catch (error) {
    set(s => {
      s.error =
        error instanceof Error ? error.message : 'Failed to load memories'
      s.loading = false
    })
  }
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

      __init__: {
        memories: () => loadMemories(set),
      },

      load: () => loadMemories(set),

      create: async (content, importance, kind): Promise<UserMemory> => {
        set(s => {
          s.saving = true
          s.error = null
        })
        try {
          const req: CreateMemoryRequest = {
            content,
            importance: importance ?? 50,
            kind: kind ?? 'fact',
            metadata: {},
          }
          const row = await ApiClient.Memory.create(req)
          set(s => {
            s.memories.unshift(row)
            s.saving = false
          })
          try {
            await emitMemoryCreated(row)
          } catch (eventError) {
            console.error(
              'Failed to emit memory created event:',
              eventError,
            )
          }
          return row
        } catch (error) {
          set(s => {
            s.error = error instanceof Error ? error.message : 'Create failed'
            s.saving = false
          })
          throw error
        }
      },

      update: async (id, patch): Promise<UserMemory> => {
        set(s => {
          s.saving = true
          s.error = null
        })
        try {
          const row = await ApiClient.Memory.update({ id, ...patch })
          set(s => {
            const idx = s.memories.findIndex(m => m.id === id)
            if (idx >= 0) s.memories[idx] = row
            s.saving = false
          })
          try {
            await emitMemoryUpdated(row)
          } catch (eventError) {
            console.error(
              'Failed to emit memory updated event:',
              eventError,
            )
          }
          return row
        } catch (error) {
          set(s => {
            s.error = error instanceof Error ? error.message : 'Update failed'
            s.saving = false
          })
          throw error
        }
      },

      remove: async (id): Promise<void> => {
        try {
          await ApiClient.Memory.delete({ id })
          set(s => {
            s.memories = s.memories.filter(m => m.id !== id)
          })
          try {
            await emitMemoryDeleted(id)
          } catch (eventError) {
            console.error(
              'Failed to emit memory deleted event:',
              eventError,
            )
          }
        } catch (error) {
          set(s => {
            s.error = error instanceof Error ? error.message : 'Delete failed'
          })
          throw error
        }
      },

      removeAll: async (): Promise<number> => {
        try {
          const body: DeleteAllResponse = await ApiClient.Memory.deleteAll()
          set(s => {
            s.memories = []
          })
          try {
            await emitMemoryAllCleared(body.deleted)
          } catch (eventError) {
            console.error(
              'Failed to emit memory all-cleared event:',
              eventError,
            )
          }
          return body.deleted
        } catch (error) {
          set(s => {
            s.error =
              error instanceof Error ? error.message : 'Delete-all failed'
          })
          throw error
        }
      },

      setSearchQuery: q =>
        set(s => {
          s.searchQuery = q
        }),
      setKindFilter: k =>
        set(s => {
          s.kindFilter = k
        }),
      setSourceFilter: source =>
        set(s => {
          s.sourceFilter = source
        }),

      reset: () =>
        set(s => {
          s.memories = []
          s.loading = false
          s.saving = false
          s.error = null
          s.searchQuery = ''
          s.kindFilter = null
          s.sourceFilter = null
        }),
    })),
  ),
)
