import type { BaseEvent } from '@/core/events'
import type { McpServer } from '@/api-client/types'

export interface McpServerCreatedEvent extends BaseEvent {
  type: 'mcp_server.created'
  data: {
    server: McpServer
  }
}

export interface McpServerUpdatedEvent extends BaseEvent {
  type: 'mcp_server.updated'
  data: {
    server: McpServer
  }
}

export interface McpServerDeletedEvent extends BaseEvent {
  type: 'mcp_server.deleted'
  data: {
    serverId: string
  }
}

export interface McpServerGroupsChangedEvent extends BaseEvent {
  type: 'mcp_server.groups_changed'
  data: {
    serverId: string
    groupIds: string[]
  }
}

export interface GroupSystemMcpServersChangedEvent extends BaseEvent {
  type: 'mcp_server.group_servers_changed'
  data: {
    groupId: string
    serverIds: string[]
  }
}

export type McpModuleEvent =
  | McpServerCreatedEvent
  | McpServerUpdatedEvent
  | McpServerDeletedEvent
  | McpServerGroupsChangedEvent
  | GroupSystemMcpServersChangedEvent

declare module '@/core/events' {
  interface AppEvents {
    'mcp_server.created': McpServerCreatedEvent
    'mcp_server.updated': McpServerUpdatedEvent
    'mcp_server.deleted': McpServerDeletedEvent
    'mcp_server.groups_changed': McpServerGroupsChangedEvent
    'mcp_server.group_servers_changed': GroupSystemMcpServersChangedEvent
  }
}
