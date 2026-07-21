import { ApiClient } from '@/api-client'
import { Permissions } from '@/api-client/permissions'
import { hasPermissionNow } from '@/core/permissions'
import type { MemoriesGet, MemoriesSet } from '../state'

export default (set: MemoriesSet, get: MemoriesGet) =>
  async (page?: number, pageSize?: number) => {
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
