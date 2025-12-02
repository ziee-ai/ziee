import { useState, useEffect, useRef } from 'react'
import { Button, Tooltip } from 'antd'
import { ToolOutlined } from '@ant-design/icons'
import { Stores } from '@/core/stores'
import { McpConfigModal } from './McpConfigModal'

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
  const appliedDefaultsRef = useRef(false)

  // Access reactive store state
  const { servers, loading } = Stores.McpServer  // MCP module store (reactive)
  const mcpStore = Stores.Chat.McpStore
  const selectedServers = mcpStore.selectedServers  // Access at component level (hooks!)
  const { currentConversationId, userDefaultsLoaded, userDefaults } = mcpStore

  // Get enabled servers (available for selection)
  const enabledServers = servers.filter(s => s.enabled)

  // Apply user defaults to pending config when:
  // 1. We're on a new conversation (no currentConversationId)
  // 2. User defaults are loaded
  // 3. We have available servers
  // 4. We haven't applied defaults yet
  useEffect(() => {
    if (
      !currentConversationId &&
      userDefaultsLoaded &&
      userDefaults &&
      enabledServers.length > 0 &&
      !appliedDefaultsRef.current
    ) {
      const availableServerIds = enabledServers.map(s => s.id)
      mcpStore.applyUserDefaultsToPending(availableServerIds)
      appliedDefaultsRef.current = true
      console.log('[McpServerSelector] Applied user defaults to pending config')
    }
    // Reset the ref when conversation changes
    if (currentConversationId) {
      appliedDefaultsRef.current = false
    }
  }, [currentConversationId, userDefaultsLoaded, userDefaults, enabledServers, mcpStore])

  // Don't show button if no enabled servers and not loading
  if (enabledServers.length === 0 && !loading) {
    return null
  }

  return (
    <>
      <Tooltip title="Configure MCP settings">
        <Button
          type={selectedServers.size > 0 ? 'primary' : 'default'}
          icon={<ToolOutlined />}
          onClick={() => setIsModalVisible(true)}
          loading={loading}
        >
          {selectedServers.size > 0 ? `${selectedServers.size} server${selectedServers.size > 1 ? 's' : ''}` : 'MCP'}
        </Button>
      </Tooltip>

      <McpConfigModal
        visible={isModalVisible}
        onClose={() => setIsModalVisible(false)}
      />
    </>
  )
}
