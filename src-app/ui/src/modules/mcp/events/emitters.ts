import { Stores } from '@/core/stores'
import type { McpServer } from '@/api-client/types'

export const emitMcpServerCreated = async (server: McpServer) => {
  await Stores.EventBus.emit({
    type: 'mcp_server.created',
    data: { server },
  })
}

export const emitMcpServerUpdated = async (server: McpServer) => {
  await Stores.EventBus.emit({
    type: 'mcp_server.updated',
    data: { server },
  })
}

export const emitMcpServerDeleted = async (serverId: string) => {
  await Stores.EventBus.emit({
    type: 'mcp_server.deleted',
    data: { serverId },
  })
}

export const emitMcpServerGroupsChanged = async (
  serverId: string,
  groupIds: string[],
) => {
  await Stores.EventBus.emit({
    type: 'mcp_server.groups_changed',
    data: { serverId, groupIds },
  })
}

export const emitGroupSystemMcpServersChanged = async (
  groupId: string,
  serverIds: string[],
) => {
  await Stores.EventBus.emit({
    type: 'mcp_server.group_servers_changed',
    data: { groupId, serverIds },
  })
}

export const emitMcpUserPolicyUpdated = async (
  allowed_transports: string[],
  user_stdio_sandbox_flavor: string | null,
) => {
  await Stores.EventBus.emit({
    type: 'mcp_user_policy.updated',
    data: { allowed_transports, user_stdio_sandbox_flavor },
  })
}
