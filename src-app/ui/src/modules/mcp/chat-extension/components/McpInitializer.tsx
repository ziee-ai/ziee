import { useEffect, useRef } from 'react'
import { Stores } from '@/core/stores'

/**
 * McpInitializer Component
 * Invisible component always mounted in toolbar_actions.
 * Responsible for applying user MCP defaults to new conversations.
 * Previously this logic lived in McpServerSelector, which was always mounted.
 */
export function McpInitializer() {
  const appliedDefaultsRef = useRef(false)

  const mcpStore = Stores.McpComposer
  const { servers } = Stores.McpServer
  const { currentConversationId, userDefaultsLoaded, userDefaults } = mcpStore

  const enabledServers = servers.filter(s => s.enabled)

  useEffect(() => {
    if (
      !currentConversationId &&
      userDefaultsLoaded &&
      enabledServers.length > 0 &&
      !appliedDefaultsRef.current
    ) {
      const availableServerIds = enabledServers.map(s => s.id)
      if (userDefaults) {
        mcpStore.applyUserDefaultsToPending(availableServerIds)
      } else {
        // No saved defaults: select all enabled servers by default
        availableServerIds.forEach(id => mcpStore.selectServer(id))
      }
      appliedDefaultsRef.current = true
    }
    if (currentConversationId) {
      appliedDefaultsRef.current = false
    }
  }, [currentConversationId, userDefaultsLoaded, userDefaults, enabledServers, mcpStore])

  return null
}
