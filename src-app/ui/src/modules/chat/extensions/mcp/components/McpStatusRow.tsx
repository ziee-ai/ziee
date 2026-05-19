import { Tag } from 'antd'
import { ToolOutlined } from '@ant-design/icons'
import { Stores } from '@/core/stores'

/**
 * McpStatusRow Component
 * Shows active MCP servers as blue tags in the status row.
 */
export function McpStatusRow() {
  const mcpStore = Stores.Chat.McpStore
  const { servers } = Stores.McpServer
  const selectedServers = mcpStore.selectedServers

  if (selectedServers.size === 0) return null

  return (
    <>
      {Array.from(selectedServers.keys()).map(serverId => {
        const server = servers.find(s => s.id === serverId)
        const label = server?.display_name || serverId

        return (
          <Tag
            key={serverId}
            color="blue"
            icon={<ToolOutlined />}
            closable
            onClose={() => mcpStore.deselectServer(serverId)}
            style={{ margin: 0 }}
          >
            {label}
          </Tag>
        )
      })}
    </>
  )
}
