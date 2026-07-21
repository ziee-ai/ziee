import { ApiClient } from '@/api-client'
import { type McpServer } from '@/api-client/types'
import type { SystemMcpServerGet, SystemMcpServerSet } from '../state'

export default (_set: SystemMcpServerSet, _get: SystemMcpServerGet) =>
  async (groupId: string): Promise<McpServer[]> => {
    try {
      // Read the group's assigned servers directly from the canonical
      // endpoint (iterating the paginated cache dropped servers not in it).
      const response = await ApiClient.Group.getSystemServers({ group_id: groupId })
      // Guard: callers `.map` the result — never hand back undefined.
      return Array.isArray(response.servers) ? response.servers : []
    } catch (error) {
      console.error('Failed to get servers for group:', error)
      throw error
    }
  }
