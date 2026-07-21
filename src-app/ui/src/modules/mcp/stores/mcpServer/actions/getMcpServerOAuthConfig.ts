import { ApiClient } from '@/api-client'
import { type McpServerOAuthConfigResponse } from '@/api-client/types'
import type { McpServerSet } from '../state'

export default (_set: McpServerSet, _get: () => never) =>
  async (serverId: string): Promise<McpServerOAuthConfigResponse | null> =>
    await ApiClient.McpServer.getOAuthConfig({ id: serverId })
