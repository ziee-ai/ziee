import { Wrench } from 'lucide-react'
import { Stores } from '@/core/stores'
import { usePlusDropdown } from '@/modules/chat/components/PlusDropdownContext'
import { PlusMenuItem } from '@/modules/chat/components/PlusMenuItem'
import { useChatPaneOrNull } from '@/modules/chat/core/pane/ChatPaneContext'

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
  const { servers, loading } = Stores.McpServer
  const mcpStore = Stores.McpComposer
  const { close } = usePlusDropdown()
  // THIS pane's conversation, resolved from its OWN store (the same
  // `useChatPaneOrNull()` pattern as McpStatusRow) — NOT the focused-pane bridge.
  // The modal edits the global `currentConversationId`, so opening it from a
  // non-focused split pane's "+" menu must point the modal at THIS pane's
  // conversation, else the toggle edits the focused pane's config (ITEM-47).
  const pane = useChatPaneOrNull()
  const chat = (pane?.store ?? Stores.Chat) as typeof Stores.Chat
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
        mcpStore.setCurrentConversation(conversation?.id ?? null)
        mcpStore.openConfigModal()
        close()
      }}
    />
  )
}
