import { ApiClient } from '@/api-client'
import { type McpServer, type UpdateMcpServerRequest } from '@/api-client/types'
import { emitMcpServerUpdated } from '@/modules/mcp/events'
import type { SystemMcpServerGet, SystemMcpServerSet } from '../state'

export default (set: SystemMcpServerSet, _get: SystemMcpServerGet) =>
  async (id: string, data: UpdateMcpServerRequest): Promise<McpServer> => {
    try {
      set({ updating: true, systemServersError: null })
      const updatedServer = await ApiClient.McpServerSystem.update({ id, ...data })
      try {
        await emitMcpServerUpdated(updatedServer)
      } catch (eventError) {
        console.error('Failed to emit mcp server updated event:', eventError)
      }
      set(state => ({
        systemServers: state.systemServers.map(server =>
          server.id === id ? updatedServer : server,
        ),
        updating: false,
      }))
      return updatedServer
    } catch (error) {
      console.error('Failed to update system server:', error)
      set({
        updating: false,
        systemServersError:
          error instanceof Error ? error.message : 'Failed to update system server',
      })
      throw error
    }
  }
