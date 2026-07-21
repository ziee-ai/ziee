import { ApiClient } from '@/api-client'
import { emitGroupSystemMcpServersChanged } from '@/modules/mcp/events'
import type { SystemMcpServerGet, SystemMcpServerSet } from '../state'

export default (_set: SystemMcpServerSet, _get: SystemMcpServerGet) =>
  async (groupId: string, serverIds: string[]): Promise<void> => {
    try {
      // Group-centric bulk update endpoint.
      await ApiClient.Group.updateSystemServers({ group_id: groupId, server_ids: serverIds })
      await emitGroupSystemMcpServersChanged(groupId, serverIds)
    } catch (error) {
      console.error('Failed to update group servers:', error)
      throw error
    }
  }
