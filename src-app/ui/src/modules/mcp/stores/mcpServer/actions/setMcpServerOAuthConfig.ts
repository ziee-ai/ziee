import { ApiClient } from '@/api-client'
import { type SetMcpServerOAuthConfigRequest } from '@/api-client/types'
import type { McpServerSet } from '../state'

export default (_set: McpServerSet, _get: () => never) =>
  async (serverId: string, config: SetMcpServerOAuthConfigRequest) => {
    await ApiClient.McpServer.setOAuthConfig({ id: serverId, ...config })
  }
