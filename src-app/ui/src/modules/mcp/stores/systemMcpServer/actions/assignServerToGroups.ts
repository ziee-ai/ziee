import { ApiClient } from '@/api-client'
import type { SystemMcpServerGet, SystemMcpServerSet } from '../state'

export default (_set: SystemMcpServerSet, _get: SystemMcpServerGet) =>
  async (serverId: string, groupIds: string[]): Promise<void> => {
    try {
      await ApiClient.McpServerSystem.assignServerToGroups({ id: serverId, group_ids: groupIds })
    } catch (error) {
      console.error('Failed to assign server to groups:', error)
      throw error
    }
  }
