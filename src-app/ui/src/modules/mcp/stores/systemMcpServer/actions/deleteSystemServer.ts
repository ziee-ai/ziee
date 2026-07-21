import { ApiClient } from '@/api-client'
import { emitMcpServerDeleted } from '@/modules/mcp/events'
import type { SystemMcpServerGet, SystemMcpServerSet } from '../state'

export default (set: SystemMcpServerSet, _get: SystemMcpServerGet) =>
  async (id: string): Promise<void> => {
    try {
      set({ deleting: true, systemServersError: null })
      await ApiClient.McpServerSystem.delete({ id })
      try {
        await emitMcpServerDeleted(id)
      } catch (eventError) {
        console.error('Failed to emit mcp server deleted event:', eventError)
      }
      set(state => ({
        systemServers: state.systemServers.filter(server => server.id !== id),
        systemServersTotal: state.systemServersTotal - 1,
        deleting: false,
      }))
    } catch (error) {
      console.error('Failed to delete system server:', error)
      set({
        deleting: false,
        systemServersError:
          error instanceof Error ? error.message : 'Failed to delete system server',
      })
      throw error
    }
  }
