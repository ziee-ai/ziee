import { ApiClient } from '@/api-client'
import type { GroupSystemMcpServersWidgetSet, GroupSystemMcpServersWidgetGet } from '../state'

export default (set: GroupSystemMcpServersWidgetSet, get: GroupSystemMcpServersWidgetGet) =>
  async (): Promise<void> => {
    const state = get()
    if (state.serversLoading) return
    if (state.serversInitialized && !state.serversError) return
    set(s => {
      s.serversLoading = true
      s.serversError = null
    })
    try {
      const response = await ApiClient.McpServerSystem.list({ page: 1, per_page: 1000 })
      set(s => {
        // Defensive: `allServers` is iterated/`.length`-read downstream — never
        // assign a non-array (a malformed/empty response would crash the row).
        s.allServers = Array.isArray(response.servers) ? response.servers : []
        s.serversLoading = false
        s.serversError = null
        s.serversInitialized = true
      })
    } catch (error) {
      console.error('Failed to load servers:', error)
      set(s => {
        s.serversLoading = false
        s.serversError = error instanceof Error ? error.message : 'Failed to load servers'
      })
      throw error
    }
  }
