import { Wrench } from 'lucide-react'
import { Stores } from '@/core/stores'
import { McpConfigModal } from '@/modules/mcp/components/McpConfigModal'
import { usePlusDropdown } from '@/modules/chat/components/PlusDropdownContext'
import { cn } from '@/lib/utils'

/**
 * McpMenuItem Component
 * Menu item inside the + dropdown for configuring MCP tools & servers
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
    <>
      <div
        className={cn(
          "flex items-center gap-2 px-3 py-2 rounded-md cursor-pointer",
          "text-foreground",
          "hover:bg-muted"
        )}
        style={{ minWidth: 180 }}
        onClick={() => { mcpStore.openConfigModal(); close() }}
      >
        <Wrench style={{ fontSize: 16 }} />
        <span className="text-sm">MCP tools & servers</span>
      </div>

      <McpConfigModal />
    </>
  )
}
