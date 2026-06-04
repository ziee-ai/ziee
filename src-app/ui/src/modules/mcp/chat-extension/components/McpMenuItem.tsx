import { theme } from 'antd'
import { ToolOutlined } from '@ant-design/icons'
import { Stores } from '@/core/stores'
import { McpConfigModal } from '@/modules/mcp/components/McpConfigModal'
import { usePlusDropdown } from '@/modules/chat/components/PlusDropdownContext'

/**
 * McpMenuItem Component
 * Menu item inside the + dropdown for configuring MCP tools & servers
 */
export function McpMenuItem() {
  const { token } = theme.useToken()
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
        className="flex items-center gap-2 px-3 py-2 rounded-md cursor-pointer"
        style={{ color: token.colorTextBase, minWidth: 180 }}
        onClick={() => { mcpStore.openConfigModal(); close() }}
        onMouseEnter={e => {
          e.currentTarget.style.backgroundColor = token.colorFillSecondary
        }}
        onMouseLeave={e => {
          e.currentTarget.style.backgroundColor = 'transparent'
        }}
      >
        <ToolOutlined style={{ fontSize: 16 }} />
        <span style={{ fontSize: 14 }}>MCP tools & servers</span>
      </div>

      <McpConfigModal />
    </>
  )
}
