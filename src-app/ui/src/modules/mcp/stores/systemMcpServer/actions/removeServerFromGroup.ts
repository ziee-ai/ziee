import { ApiClient } from '@/api-client'
import type { SystemMcpServerGet, SystemMcpServerSet } from '../state'

export default (_set: SystemMcpServerSet, _get: SystemMcpServerGet) =>
  async (serverId: string, groupId: string): Promise<void> => {
    try {
      await ApiClient.McpServerSystem.removeServerFromGroup({ id: serverId, group_id: groupId })
    } catch (error) {
      console.error('Failed to remove server from group:', error)
      throw error
    }
  }
