import { useState } from 'react'
import { Button, Tooltip } from 'antd'
import { ToolOutlined } from '@ant-design/icons'
import { Stores } from '@/core/stores'
import { McpServerSelectionModal } from './McpServerSelectionModal'

/**
 * MCP Server Selector Component
 *
 * Toolbar button for selecting MCP servers and tools.
 * Only visible when there are available (enabled + active) servers with tools.
 *
 * Button appearance:
 * - Default (gray) when no servers selected
 * - Primary (blue) when servers are selected
 */
export function McpServerSelector() {
  const [isModalVisible, setIsModalVisible] = useState(false)

  // Access reactive store state
  const { servers, loading } = Stores.McpServer  // MCP module store (reactive)
  const mcpStore = Stores.Chat.McpStore
  const selectedServers = mcpStore.selectedServers  // Access at component level (hooks!)

  // Get enabled servers (available for selection)
  const enabledServers = servers.filter(s => s.enabled)

  // Don't show button if no enabled servers and not loading
  if (enabledServers.length === 0 && !loading) {
    return null
  }

  return (
    <>
      <Tooltip title="Select MCP tools">
        <Button
          type={selectedServers.size > 0 ? 'primary' : 'default'}
          icon={<ToolOutlined />}
          onClick={() => setIsModalVisible(true)}
          loading={loading}
        >
          {selectedServers.size > 0 ? `${selectedServers.size} server${selectedServers.size > 1 ? 's' : ''}` : 'MCP Tools'}
        </Button>
      </Tooltip>

      <McpServerSelectionModal
        visible={isModalVisible}
        onClose={() => setIsModalVisible(false)}
      />
    </>
  )
}
