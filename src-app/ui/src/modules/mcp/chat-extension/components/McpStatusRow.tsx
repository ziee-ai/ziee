import { Tag } from '@ziee/kit'
import { Wrench } from 'lucide-react'
import { pendingConversationKey } from '@/modules/mcp/stores/mcpComposer'
import { useChatPaneOrNull } from '@/modules/chat/core/pane/ChatPaneContext'
import { McpComposer } from '@/modules/mcp/stores/mcpComposer'
import { McpServer } from '@/modules/mcp/stores/mcpServer'
import { Chat } from '@/modules/chat/core/stores/chatBridge'

/**
 * McpStatusRow Component
 * Shows active MCP servers as blue tags in the status row.
 *
 * Per-pane (ITEM-47): the visible selection resolves from THIS pane's conversation's
 * own config (via the per-conversation `conversationConfigs` Map), NOT the single
 * global-active `selectedServers` — which reflects the focused/last-loaded
 * conversation and so leaked the focused pane's servers into every pane's chips.
 * The pane's conversation is resolved from its OWN store (`useChatPaneOrNull()`,
 * the proven ConversationPage pattern) — not the focused-pane bridge — so a split
 * pane's chips reflect ITS conversation; removing a chip edits THAT conversation
 * via `deselectServerForConversation`, never the focused pane's.
 */
export function McpStatusRow() {
  const mcpStore = McpComposer
  const { servers } = McpServer
  // Reactive: re-render when any conversation's selection changes.
  const { conversationConfigs } = mcpStore
  // This pane's OWN conversation via its own store (the proven ConversationPage
  // pattern) rather than the focused-pane bridge — so a split pane's chips reflect
  // ITS conversation, not the focused one.
  const pane = useChatPaneOrNull()
  const chat = (pane?.store ?? Chat) as typeof Chat
  const paneId = pane?.paneId ?? null
  const conversation = chat.conversation
  // A new chat reads THIS pane's own pending config (ITEM-51), so a pending MCP
  // selection in one new-chat pane never shows in the other.
  const convKey = conversation?.id ?? pendingConversationKey(paneId)
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
              // Edit THIS pane's conversation (or its own pending buffer for a new
              // chat), not the global-active one.
              mcpStore.deselectServerForConversation(conversation?.id ?? null, serverId, paneId)
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
