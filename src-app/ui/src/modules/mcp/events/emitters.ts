import type { McpServer } from '@/api-client/types'
import { EventBus } from '@ziee/framework/stores'

export const emitMcpServerCreated = async (server: McpServer) => {
  await EventBus.emit({
    type: 'mcp_server.created',
    data: { server },
  })
}

export const emitMcpServerUpdated = async (server: McpServer) => {
  await EventBus.emit({
    type: 'mcp_server.updated',
    data: { server },
  })
}

export const emitMcpServerDeleted = async (serverId: string) => {
  await EventBus.emit({
    type: 'mcp_server.deleted',
    data: { serverId },
  })
}

export const emitMcpServerGroupsChanged = async (
  serverId: string,
  groupIds: string[],
) => {
  await EventBus.emit({
    type: 'mcp_server.groups_changed',
    data: { serverId, groupIds },
  })
}

export const emitGroupSystemMcpServersChanged = async (
  groupId: string,
  serverIds: string[],
) => {
  await EventBus.emit({
    type: 'mcp_server.group_servers_changed',
    data: { groupId, serverIds },
  })
}

export const emitMcpUserPolicyUpdated = async (
  allowed_transports: string[],
  user_stdio_sandbox_flavor: string | null,
) => {
  await EventBus.emit({
    type: 'mcp_user_policy.updated',
    data: { allowed_transports, user_stdio_sandbox_flavor },
  })
}
