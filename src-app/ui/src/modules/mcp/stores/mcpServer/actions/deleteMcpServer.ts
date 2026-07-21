import { ApiClient } from '@/api-client'
import { emitMcpServerDeleted } from '@/modules/mcp/events'
import { useSystemMcpServersStore } from '@/modules/mcp/stores/SystemMcpServer.store'
import type { McpServerGet, McpServerSet } from '../state'

export default (set: McpServerSet, _get: McpServerGet) =>
  async (serverId: string): Promise<void> => {
    set(draft => {
      draft.operationsLoading.set(serverId, true)
      draft.error = null
    })
    try {
      await ApiClient.McpServer.delete({ id: serverId })
      try {
        await emitMcpServerDeleted(serverId)
      } catch (eventError) {
        console.error('Failed to emit mcp server deleted event:', eventError)
      }
      set(draft => {
        draft.operationsLoading.delete(serverId)
      })
      useSystemMcpServersStore.setState(state => ({
        ...state,
        systemServers: state.systemServers.filter(server => server.id !== serverId),
      }))
    } catch (error) {
      console.error('MCP server deletion failed:', error)
      set(draft => {
        draft.operationsLoading.delete(serverId)
        draft.error = error instanceof Error ? error.message : 'Failed to delete MCP server'
      })
      throw error
    }
  }
