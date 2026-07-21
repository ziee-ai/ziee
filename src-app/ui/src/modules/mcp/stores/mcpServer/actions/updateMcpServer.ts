import { ApiClient } from '@/api-client'
import { type McpServer, type UpdateMcpServerRequest } from '@/api-client/types'
import { emitMcpServerUpdated } from '@/modules/mcp/events'
import { useSystemMcpServersStore } from '@/modules/mcp/stores/systemMcpServer'
import type { McpServerGet, McpServerSet } from '../state'

export default (set: McpServerSet, _get: McpServerGet) =>
  async (serverId: string, data: UpdateMcpServerRequest): Promise<McpServer> => {
    set(draft => {
      draft.operationsLoading.set(serverId, true)
      draft.error = null
    })
    try {
      const updatedServer = await ApiClient.McpServer.update({ id: serverId, ...data })
      try {
        await emitMcpServerUpdated(updatedServer)
      } catch (eventError) {
        console.error('Failed to emit mcp server updated event:', eventError)
      }
      set(draft => {
        draft.operationsLoading.delete(serverId)
      })
      // Mirror into the system store if the row lives there (plain, no immer).
      useSystemMcpServersStore.setState(state => {
        const index = state.systemServers.findIndex(server => server.id === updatedServer.id)
        if (index >= 0) {
          return {
            ...state,
            systemServers: state.systemServers.map(server =>
              server.id === updatedServer.id ? updatedServer : server,
            ),
          }
        }
        return state
      })
      return updatedServer
    } catch (error) {
      console.error('MCP server update failed:', error)
      set(draft => {
        draft.operationsLoading.delete(serverId)
        draft.error = error instanceof Error ? error.message : 'Failed to update MCP server'
      })
      throw error
    }
  }
