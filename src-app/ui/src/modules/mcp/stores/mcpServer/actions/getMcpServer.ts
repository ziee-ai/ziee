import { ApiClient } from '@/api-client'
import { type McpServer } from '@/api-client/types'
import { useSystemMcpServersStore } from '@/modules/mcp/stores/SystemMcpServer.store'
import type { McpServerGet, McpServerSet } from '../state'

export default (set: McpServerSet, _get: McpServerGet) =>
  async (serverId: string): Promise<McpServer> => {
    const server = await ApiClient.McpServer.get({ id: serverId })
    set(draft => {
      const index = draft.servers.findIndex(s => s.id === server.id)
      if (index >= 0) draft.servers[index] = server
    })
    useSystemMcpServersStore.setState(state => {
      const index = state.systemServers.findIndex(s => s.id === server.id)
      if (index >= 0) {
        return {
          ...state,
          systemServers: state.systemServers.map(s => (s.id === server.id ? server : s)),
        }
      }
      return state
    })
    return server
  }
