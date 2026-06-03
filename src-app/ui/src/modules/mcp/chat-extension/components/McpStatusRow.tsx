import { Tag } from 'antd'
import { ToolOutlined } from '@ant-design/icons'
import { Stores } from '@/core/stores'

/**
 * McpStatusRow Component
 * Shows active MCP servers as blue tags in the status row.
 */
export function McpStatusRow() {
  const mcpStore = Stores.McpComposer
  const { servers } = Stores.McpServer
  // Extract all store properties at the top — store proxy uses hooks
  const { selectedServers, currentConversationId } = mcpStore

  // Compute derived values during render (not inside event handlers)
  const enabledServerIds = servers.filter(s => s.enabled).map(s => s.id)

  // Only show servers that are currently enabled (filter out disabled/removed servers)
  const visibleServerIds = Array.from(selectedServers.keys()).filter(serverId =>
    servers.some(s => s.id === serverId && !s.is_built_in)
  )

  if (visibleServerIds.length === 0) return null

  return (
    <>
      {visibleServerIds.map(serverId => {
        const server = servers.find(s => s.id === serverId)!

        return (
          <Tag
            key={serverId}
            color="blue"
            icon={<ToolOutlined />}
            closable
            onClose={async () => {
              mcpStore.deselectServer(serverId)
              if (currentConversationId) {
                // Existing conversation: persist to conversation config
                await mcpStore.saveConversationConfig(currentConversationId, enabledServerIds)
              } else {
                // New conversation: persist as user defaults so applyUserDefaultsToPending
                // restores the correct selection after reload
                await mcpStore.saveUserDefaults(null, enabledServerIds)
              }
            }}
            style={{ margin: 0 }}
            data-testid={`mcp-chip-${serverId}`}
          >
            {server.display_name}
          </Tag>
        )
      })}
    </>
  )
}
