import { Wrench } from 'lucide-react'
import { Stores } from '@ziee/framework/stores'
import { usePlusDropdown } from '@/modules/chat/components/PlusDropdownContext'
import { PlusMenuItem } from '@/modules/chat/components/PlusMenuItem'

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
        mcpStore.openConfigModal()
        close()
      }}
    />
  )
}
