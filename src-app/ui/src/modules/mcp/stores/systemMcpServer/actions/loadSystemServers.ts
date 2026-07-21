import { ApiClient } from '@/api-client'
import type { SystemMcpServerGet, SystemMcpServerSet } from '../state'

export default (set: SystemMcpServerSet, get: SystemMcpServerGet) =>
  async (page?: number, pageSize?: number): Promise<void> => {
    const state = get()
    if (state.systemServersInitialized && state.systemServersLoading && !page) return
    try {
      const requestPage = page || state.systemServersPage
      const requestPageSize = pageSize || state.systemServersPageSize
      set({ systemServersLoading: true, systemServersError: null })
      const response = await ApiClient.McpServerSystem.list({
        page: requestPage,
        per_page: requestPageSize,
        ...(state.searchTerm ? { search: state.searchTerm } : {}),
        ...(state.statusFilter !== 'all' ? { status: state.statusFilter } : {}),
      })
      set({
        systemServers: response.servers,
        systemServersTotal: response.total,
        systemServersPage: response.page,
        systemServersPageSize: response.per_page,
        systemServersInitialized: true,
        systemServersLoading: false,
        systemServersError: null,
      })
    } catch (error) {
      console.error('Failed to load system servers:', error)
      set({
        systemServersLoading: false,
        systemServersError:
          error instanceof Error ? error.message : 'Failed to load system servers',
      })
      throw error
    }
  }
