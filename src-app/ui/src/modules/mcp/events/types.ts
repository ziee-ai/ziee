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

/**
 * Admin saved a new MCP user policy. Subscribed by:
 *   - the McpServerDrawer (re-render the transport dropdown / sandbox
 *     info alert against the new allowed_transports + flavor),
 *   - McpServersSettings (re-render the Add button gate),
 *   - the Hub MCP tab registration (`shouldRender` re-evaluates).
 *
 * Event verb is `.updated` to match the peer convention
 * (`mcp_server.updated`, `mcp_server.created`, etc.) — admin actions
 * on resources use `.updated`, not `.changed`.
 */
export interface McpUserPolicyUpdatedEvent extends BaseEvent {
  type: 'mcp_user_policy.updated'
  data: {
    allowed_transports: string[]
    user_stdio_sandbox_flavor: string | null
  }
}

export type McpModuleEvent =
  | McpServerCreatedEvent
  | McpServerUpdatedEvent
  | McpServerDeletedEvent
  | McpServerGroupsChangedEvent
  | GroupSystemMcpServersChangedEvent
  | McpUserPolicyUpdatedEvent

declare module '@/core/events' {
  interface AppEvents {
    'mcp_server.created': McpServerCreatedEvent
    'mcp_server.updated': McpServerUpdatedEvent
    'mcp_server.deleted': McpServerDeletedEvent
    'mcp_server.groups_changed': McpServerGroupsChangedEvent
    'mcp_server.group_servers_changed': GroupSystemMcpServersChangedEvent
    'mcp_user_policy.updated': McpUserPolicyUpdatedEvent
  }
}
