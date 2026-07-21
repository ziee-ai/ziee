import { ApiClient } from '@/api-client'
import type { McpServerSet } from '../state'

export default (_set: McpServerSet, _get: () => never) =>
  async (serverId: string) => {
    await ApiClient.McpServer.deleteOAuthConfig({ id: serverId })
  }
