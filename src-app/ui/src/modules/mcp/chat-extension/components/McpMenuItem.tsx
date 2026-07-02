import { Wrench } from 'lucide-react'
import { Stores } from '@/core/stores'
import { usePlusDropdown } from '@/modules/chat/components/PlusDropdownContext'
import { cn } from '@/lib/utils'

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
    <div
      data-testid="chat-mcp-menu-item"
      className={cn(
        "flex items-center gap-2 px-3 py-1.5 rounded-md cursor-pointer",
        "text-foreground",
        "hover:bg-muted"
      )}
      onClick={() => { mcpStore.openConfigModal(); close() }}
    >
      <Wrench className="size-4" />
      <span className="text-sm">MCP tools & servers</span>
    </div>
  )
}
