import { Wrench } from 'lucide-react'
import { usePlusDropdown } from '@/modules/chat/components/PlusDropdownContext'
import { PlusMenuItem } from '@/modules/chat/components/PlusMenuItem'
import { useChatPaneOrNull } from '@/modules/chat/core/pane/ChatPaneContext'
import { McpComposer as McpComposerStore } from '@/modules/mcp/stores/mcpComposer'
import { McpServer } from '@/modules/mcp/stores/mcpServer'
import { Chat } from '@/modules/chat/core/stores/chatBridge'

/**
 * McpMenuItem Component
 * Menu item inside the + dropdown for configuring MCP tools & servers.
 *
 * Opening it flips the McpComposer modal-open state; the McpConfigModal itself
 * is hosted from an always-mounted composer slot (see the extension's
 * `input_area_suffix` registration), NOT here. The "+" dropdown unmounts its
 * items when it closes (which this onClick triggers via `close()`), so a modal
 * rendered inside this item would be torn down before it could appear.
 */
export function McpMenuItem() {
  const { servers, loading } = McpServer
  const mcpStore = McpComposerStore
  const { close } = usePlusDropdown()
  // THIS pane's conversation, resolved from its OWN store (the same
  // `useChatPaneOrNull()` pattern as McpStatusRow) — NOT the focused-pane bridge.
  // The modal edits the global `currentConversationId`, so opening it from a
  // non-focused split pane's "+" menu must point the modal at THIS pane's
  // conversation, else the toggle edits the focused pane's config (ITEM-47).
  const pane = useChatPaneOrNull()
  const chat = (pane?.store ?? Chat) as typeof Chat
  const paneId = pane?.paneId ?? null
  const conversation = chat.conversation

  const enabledServers = servers.filter(s => s.enabled)

  if (enabledServers.length === 0 && !loading) {
    return null
  }

  return (
    <PlusMenuItem
      data-testid="chat-mcp-menu-item"
      aria-label="MCP tools & servers"
      icon={<Wrench />}
      label="MCP tools & servers"
      onClick={() => {
        // Bind the shared modal to THIS pane's conversation AND pane id (ITEM-51),
        // so a new-chat toggle edits this pane's own pending config.
        mcpStore.setCurrentConversation(conversation?.id ?? null, paneId)
        mcpStore.openConfigModal()
        close()
      }}
    />
  )
}
