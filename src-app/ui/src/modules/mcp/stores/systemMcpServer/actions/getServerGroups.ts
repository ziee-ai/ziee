import { ApiClient } from '@/api-client'
import type { SystemMcpServerGet, SystemMcpServerSet } from '../state'

export default (_set: SystemMcpServerSet, _get: SystemMcpServerGet) =>
  async (serverId: string): Promise<string[]> => {
    try {
      return await ApiClient.McpServerSystem.getServerGroups({ id: serverId })
    } catch (error) {
      console.error('Failed to get server groups:', error)
      throw error
    }
  }
