import { ApiClient } from '@/api-client'
import { type CreateMcpServerRequest, type McpServerWithHealthWarning } from '@/api-client/types'
import { emitMcpServerCreated } from '@/modules/mcp/events'
import type { SystemMcpServerGet, SystemMcpServerSet } from '../state'

export default (set: SystemMcpServerSet, _get: SystemMcpServerGet) =>
  async (data: CreateMcpServerRequest): Promise<McpServerWithHealthWarning> => {
    try {
      set({ creating: true, systemServersError: null })
      // Response is flattened: McpServer fields at top level + optional
      // `connection_warning` sibling (health-check-on-create).
      const wrapped = await ApiClient.McpServerSystem.create(data)
      const { connection_warning: _w, ...newServer } = wrapped
      try {
        await emitMcpServerCreated(newServer)
      } catch (eventError) {
        console.error('Failed to emit mcp server created event:', eventError)
      }
      set(state => ({
        systemServers: [...state.systemServers, newServer],
        systemServersTotal: state.systemServersTotal + 1,
        creating: false,
      }))
      return wrapped
    } catch (error) {
      console.error('Failed to create system server:', error)
      set({
        creating: false,
        systemServersError:
          error instanceof Error ? error.message : 'Failed to create system server',
      })
      throw error
    }
  }
