import { ApiClient } from '@/api-client'
import type {
  CreateMemoryRequest,
  DeleteAllResponse,
  UpdateMemoryRequest,
  UserMemory,
} from '@/api-client/types'
import { Permissions } from '@/api-client/permissions'
import { hasPermissionNow } from '@/core/permissions'
import { defineStore } from '@ziee/framework/store-kit'
import {
  emitMemoryAllCleared,
  emitMemoryCreated,
  emitMemoryDeleted,
  emitMemoryUpdated,
} from '@/modules/memory/events'

// Debounce timer for search-query reloads — keystrokes within 250ms coalesce.
let searchDebounce: ReturnType<typeof setTimeout> | null = null

export const Memories = defineStore('Memories', {
  immer: true,
  state: {
    memories: [] as UserMemory[],
    loading: false,
    saving: false,
    error: null as string | null,
    searchQuery: '',
    kindFilter: null as string | null,
    sourceFilter: null as string | null,
    // Pagination state — drives MyMemoriesSection's <Pagination>.
    currentPage: 1,
    pageSize: 10,
    total: 0,
  },
  actions: (set, get) => {
    const load = async (page?: number, pageSize?: number) => {
      // `sync:reconnect` fires for every store regardless of audience; skip the
      // refetch for users without `memory::read` (the endpoint would 403).
      if (!hasPermissionNow(Permissions.MemoryRead)) return
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
          // Server-side filters (ILIKE + exact kind/source). Empty/null omitted.
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
          s.error = error instanceof Error ? error.message : 'Failed to load memories'
          s.loading = false
        })
      }
    }
    return {
      load,
      create: async (
        content: string,
        importance?: number,
        kind?: string,
      ): Promise<UserMemory> => {
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
            console.error('Failed to emit memory created event:', eventError)
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
      update: async (
        id: string,
        patch: Omit<UpdateMemoryRequest, never>,
      ): Promise<UserMemory> => {
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
            console.error('Failed to emit memory updated event:', eventError)
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
      remove: async (id: string): Promise<void> => {
        try {
          await ApiClient.Memory.delete({ id })
          set(s => {
            s.memories = s.memories.filter(m => m.id !== id)
            s.total = Math.max(0, s.total - 1)
          })
          try {
            await emitMemoryDeleted(id)
          } catch (eventError) {
            console.error('Failed to emit memory deleted event:', eventError)
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
            console.error('Failed to emit memory all-cleared event:', eventError)
          }
          return body.deleted
        } catch (error) {
          set(s => {
            s.error = error instanceof Error ? error.message : 'Delete-all failed'
          })
          throw error
        }
      },
      // Filter setters reset to page 1 and reload. Search is debounced (250ms);
      // select-style filters fire immediately.
      setSearchQuery: (q: string) => {
        set(s => {
          s.searchQuery = q
          s.currentPage = 1
        })
        if (searchDebounce) clearTimeout(searchDebounce)
        searchDebounce = setTimeout(() => {
          void load(1)
        }, 250)
      },
      setKindFilter: (k: string | null) => {
        set(s => {
          s.kindFilter = k
          s.currentPage = 1
        })
        void load(1)
      },
      setSourceFilter: (source: string | null) => {
        set(s => {
          s.sourceFilter = source
          s.currentPage = 1
        })
        void load(1)
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
    }
  },
  init: ({ on, actions }) => {
    // Paginated list; load() reloads the current page (surfacing remote
    // creates/edits/deletes on it; a bulk-clear arrives as a nil-id Delete).
    // load() is permission-gated internally.
    const reload = () => void actions.load()
    on('sync:memory', reload)
    on('sync:reconnect', reload)
    void actions.load()
  },
})

export const useMemoriesStore = Memories.store
