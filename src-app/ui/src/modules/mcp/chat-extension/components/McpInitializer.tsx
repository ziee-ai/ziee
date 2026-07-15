import { useEffect, useRef } from 'react'
import { Stores } from '@ziee/framework/stores'
import { useChatPaneOrNull } from '@/modules/chat/core/pane/ChatPaneContext'

/**
 * McpInitializer Component
 * Invisible component always mounted in toolbar_actions.
 * Responsible for applying user MCP defaults to new conversations.
 * Previously this logic lived in McpServerSelector, which was always mounted.
 *
 * Per-pane (ITEM-51): it is mounted once PER PANE (toolbar_actions), so it seeds
 * THIS pane's own new-chat defaults into THIS pane's own pending buffer — gated on
 * the pane's OWN conversation (not the single global `currentConversationId`
 * pointer) and threading the pane's id into the pending write. Single-pane (null
 * pane) keeps the bare key + the primary conversation, so behaviour is unchanged.
 */
export function McpInitializer() {
  const appliedDefaultsRef = useRef(false)

  const pane = useChatPaneOrNull()
  const paneId = pane?.paneId ?? null
  const chat = (pane?.store ?? Stores.Chat) as typeof Stores.Chat
  const paneConvId = chat.conversation?.id ?? null

  const mcpStore = Stores.McpComposer
  const { servers } = Stores.McpServer
  const { userDefaultsLoaded, userDefaults } = mcpStore

  const enabledServers = servers.filter(s => s.enabled)

  useEffect(() => {
    if (
      !paneConvId &&
      userDefaultsLoaded &&
      enabledServers.length > 0 &&
      !appliedDefaultsRef.current
    ) {
      const availableServerIds = enabledServers.map(s => s.id)
      if (userDefaults) {
        mcpStore.applyUserDefaultsToPending(availableServerIds, paneId)
      } else {
        // No saved defaults: select all enabled servers by default (into THIS
        // pane's own pending config).
        availableServerIds.forEach(id => mcpStore.selectServer(id, [], paneId))
      }
      appliedDefaultsRef.current = true
    }
    if (paneConvId) {
      appliedDefaultsRef.current = false
    }
  }, [paneConvId, paneId, userDefaultsLoaded, userDefaults, enabledServers, mcpStore])

  return null
}
