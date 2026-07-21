import { ApiClient } from '@/api-client'
import type { McpServerGet, McpServerSet } from '../state'

export default (set: McpServerSet, get: McpServerGet) =>
  async (page: number, pageSize: number, searchTerm: string, statusFilter: string) => {
    const state = get()
    if (state.loading) return

    try {
      set(draft => {
        draft.loading = true
        draft.error = null
      })
      const response = await ApiClient.McpServer.listAccessible({
        page,
        per_page: pageSize,
        ...(searchTerm ? { search: searchTerm } : {}),
        ...(statusFilter !== 'all' ? { status: statusFilter } : {}),
      })
      set(draft => {
        // Defensive: never assign a non-array (the list is iterated/`.length`-read).
        draft.servers = Array.isArray(response.servers) ? response.servers : []
        draft.total = response.total
        draft.currentPage = response.page
        draft.pageSize = response.per_page
        draft.isInitialized = true
        draft.loading = false
        draft.error = null
      })
    } catch (error) {
      console.error('MCP servers loading failed:', error)
      set(draft => {
        draft.loading = false
        draft.error = error instanceof Error ? error.message : 'Failed to load MCP servers'
      })
      throw error
    }
  }
