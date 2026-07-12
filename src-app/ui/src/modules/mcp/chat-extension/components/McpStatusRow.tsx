import { Tag } from '@/components/ui'
import { Wrench } from 'lucide-react'
import { Stores } from '@/core/stores'
import { PENDING_CONVERSATION_KEY } from '@/modules/mcp/stores/McpComposer.store'

/**
 * McpStatusRow Component
 * Shows active MCP servers as blue tags in the status row.
 *
 * Per-pane (ITEM-47): the visible selection resolves from THIS pane's conversation's
 * own config (via the per-conversation `conversationConfigs` Map), NOT the single
 * global-active `selectedServers` — which reflects the focused/last-loaded
 * conversation and so leaked the focused pane's servers into every pane's chips.
 * The pane's conversation is resolved through the reactive `Stores.Chat` bridge
 * (reactive reads resolve to the pane subtree); removing a chip edits THAT
 * conversation via `deselectServerForConversation`, never the focused pane's.
 */
export function McpStatusRow() {
  const mcpStore = Stores.McpComposer
  const { servers } = Stores.McpServer
  // Reactive: re-render when any conversation's selection changes.
  const { conversationConfigs } = mcpStore
  // This pane's conversation (bridge-resolved); null → the pending new-chat buffer.
  const conversation = Stores.Chat.conversation
  const convKey = conversation?.id ?? PENDING_CONVERSATION_KEY
  const selectedServers = conversationConfigs.get(convKey)?.selectedServers

  // Compute derived values during render (not inside event handlers)
  const enabledServerIds = servers.filter(s => s.enabled).map(s => s.id)

  // Only show servers that are currently enabled (filter out disabled/removed servers)
  const visibleServerIds = selectedServers
    ? Array.from(selectedServers.keys()).filter(serverId =>
        servers.some(s => s.id === serverId && !s.is_built_in),
      )
    : []

  if (visibleServerIds.length === 0) return null

  return (
    <>
      {visibleServerIds.map(serverId => {
        const server = servers.find(s => s.id === serverId)!

        return (
          <Tag variant="outline"
            key={serverId}
            tone="info"
            icon={<Wrench />}
            onClose={async () => {
              // Edit THIS pane's conversation, not the global-active one.
              mcpStore.deselectServerForConversation(conversation?.id ?? null, serverId)
              if (conversation?.id) {
                // Existing conversation: persist to conversation config
                await mcpStore.saveConversationConfig(conversation.id, enabledServerIds)
              } else {
                // New conversation: persist as user defaults so applyUserDefaultsToPending
                // restores the correct selection after reload
                await mcpStore.saveUserDefaults(null, enabledServerIds)
              }
            }}
            closeLabel="Remove"
            className="m-0"
            data-testid={`mcp-chip-${serverId}`}
          >
            {server.display_name}
          </Tag>
        )
      })}
    </>
  )
}
