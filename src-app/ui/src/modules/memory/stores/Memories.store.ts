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

  // Pagination state — drives MyMemoriesSection's <Pagination>.
  // Backend `Memory.list` accepts `page` + `per_page` and returns
  // `MemoryListResponse { items, total, page, per_page }`.
  currentPage: number
  pageSize: number
  total: number

  __init__: {
    memories: () => Promise<void>
  }

  load: (page?: number, pageSize?: number) => Promise<void>
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
  get: () => MemoriesStore,
  page?: number,
  pageSize?: number,
) => {
  const state = get()
  const nextPage = page ?? state.currentPage
  const nextPageSize = pageSize ?? state.pageSize
  set(s => {
    s.loading = true
    s.error = null
  })
  try {
    const resp = await ApiClient.Memory.list({
      page: nextPage,
      per_page: nextPageSize,
      // Server-side filters — backend ILIKE + exact-match on kind/source.
      // Empty/null values are omitted so the server short-circuits.
      ...(state.searchQuery ? { search: state.searchQuery } : {}),
      ...(state.kindFilter ? { kind: state.kindFilter } : {}),
      ...(state.sourceFilter ? { source: state.sourceFilter } : {}),
    })
    set(s => {
      s.memories = resp.items
      s.total = resp.total
      s.currentPage = resp.page
      s.pageSize = resp.per_page
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

/**
 * Debounce timer for search-query reloads — keystrokes within
 * 250ms coalesce into a single backend hit.
 */
let searchDebounce: ReturnType<typeof setTimeout> | null = null

export const useMemoriesStore = create<MemoriesStore>()(
  subscribeWithSelector(
    immer((set, get) => ({
      memories: [],
      loading: false,
      saving: false,
      error: null,
      searchQuery: '',
      kindFilter: null,
      sourceFilter: null,
      currentPage: 1,
      pageSize: 10,
      total: 0,

      __init__: {
        memories: () => loadMemories(set, get),
      },

      load: (page?: number, pageSize?: number) =>
        loadMemories(set, get, page, pageSize),

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
            s.total += 1
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
            s.total = Math.max(0, s.total - 1)
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
            s.total = 0
            s.currentPage = 1
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

      // Filter setters all reset to page 1 and reload from the
      // server. Search is debounced (250ms) so keystrokes don't
      // hammer the backend; select-style filters fire immediately.
      setSearchQuery: q => {
        set(s => {
          s.searchQuery = q
          s.currentPage = 1
        })
        if (searchDebounce) clearTimeout(searchDebounce)
        searchDebounce = setTimeout(() => {
          void loadMemories(set, get, 1)
        }, 250)
      },
      setKindFilter: k => {
        set(s => {
          s.kindFilter = k
          s.currentPage = 1
        })
        void loadMemories(set, get, 1)
      },
      setSourceFilter: source => {
        set(s => {
          s.sourceFilter = source
          s.currentPage = 1
        })
        void loadMemories(set, get, 1)
      },

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
