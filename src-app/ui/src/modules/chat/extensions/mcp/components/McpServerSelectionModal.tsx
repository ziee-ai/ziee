import { useEffect, useState } from 'react'
import { Modal, Collapse, Switch, Tag, Typography, Empty, Checkbox } from 'antd'
import { ToolOutlined } from '@ant-design/icons'
import { Stores } from '@/core/stores'
import { ApiClient } from '@/api-client'
import type { Tool } from '@/api-client/types'

const { Text } = Typography
const { Panel } = Collapse

interface McpServerSelectionModalProps {
  visible: boolean
  onClose: () => void
}

/**
 * MCP Server Selection Modal
 *
 * Allows users to select which MCP servers and tools to enable for messages.
 * Features:
 * - Groups tools by MCP server
 * - Server-level toggle (enable all tools from server)
 * - Individual tool toggles
 * - Auto-initializes with all available tools selected
 */
export function McpServerSelectionModal({
  visible,
  onClose,
}: McpServerSelectionModalProps) {
  const { servers } = Stores.McpServer  // Reactive access to MCP module store
  const mcpStore = Stores.Chat.McpStore
  const {selectedServers} = mcpStore  // Access at component level (hooks!)

  console.log({selectedServers})

  // Local state for tools (loaded on demand)
  const [serverTools, setServerTools] = useState<Map<string, Tool[]>>(new Map())
  const [loadingTools, setLoadingTools] = useState<Set<string>>(new Set())
  const [hasAutoSelected, setHasAutoSelected] = useState(false)

  // Get enabled servers (available for selection)
  const enabledServers = servers.filter(s => s.enabled)

  // Auto-initialize: lazy load tools and select all servers on first open
  useEffect(() => {
    if (visible) {
      // Lazy load tools for servers that don't have them yet
      enabledServers.forEach(async server => {
        if (!serverTools.has(server.id) && !loadingTools.has(server.id)) {
          setLoadingTools(prev => new Set(prev).add(server.id))

          try {
            const response = await ApiClient.McpServerRuntime.listTools({ id: server.id })
            setServerTools(prev => new Map(prev).set(server.id, response.tools))
          } catch (error) {
            console.error('[MCP Modal] Failed to load tools for server:', server.id, error)
          } finally {
            setLoadingTools(prev => {
              const next = new Set(prev)
              next.delete(server.id)
              return next
            })
          }
        }
      })

      // Auto-select all servers on first open only
      if (!hasAutoSelected && selectedServers.size === 0 && enabledServers.length > 0) {
        enabledServers.forEach(server => {
          mcpStore.selectServer(server.id, []) // Empty array = all tools
        })
        setHasAutoSelected(true)
      }
    } else {
      // Reset when modal closes
      setHasAutoSelected(false)
    }
  }, [visible, selectedServers.size, enabledServers.length, hasAutoSelected])

  // Check if a specific tool is selected
  const isToolSelected = (serverId: string, toolName: string): boolean => {
    const selection = selectedServers.get(serverId)
    if (!selection) return false
    // If tools array is empty, all tools are selected
    if (selection.tools.length === 0) return true
    // Otherwise check if tool is in the array
    return selection.tools.includes(toolName)
  }

  // Handle server-level toggle
  const handleServerToggle = (serverId: string, checked: boolean) => {
    if (checked) {
      // Select server with all tools (empty array)
      mcpStore.selectServer(serverId, [])
    } else {
      // Deselect server
      mcpStore.deselectServer(serverId)
    }
  }

  // Handle individual tool toggle
  const handleToolToggle = (serverId: string, toolName: string) => {
    const selection = selectedServers.get(serverId)

    if (!selection) {
      // Server not selected, select it with just this tool
      mcpStore.selectServer(serverId, [toolName])
      return
    }

    if (selection.tools.length === 0) {
      // All tools selected, switch to explicit selection
      const tools = serverTools.get(serverId) || []
      const otherTools = tools
        .filter(t => t.name !== toolName)
        .map(t => t.name)
      mcpStore.selectServer(serverId, otherTools)
    } else {
      // Some tools selected, toggle this one
      mcpStore.toggleServerTool(serverId, toolName)
    }
  }

  // Render server panel
  const renderServerPanel = (server: any) => {
    const tools = serverTools.get(server.id) || []
    const selection = selectedServers.get(server.id)

    return (
      <Panel
        key={server.id}
        header={
          <div className="flex items-center justify-between w-full" onClick={(e) => e.stopPropagation()}>
            <div className="flex items-center gap-2">
              <Switch
                checked={!!selection}
                onChange={(checked) => handleServerToggle(server.id, checked)}
                size="small"
              />
              <Text strong>{server.display_name}</Text>
              <Tag color={server.user_id ? 'blue' : 'green'} className="text-xs">
                {server.user_id ? 'User' : 'System'}
              </Tag>
            </div>
            <Text type="secondary" className="text-xs">
              {tools.length} tool{tools.length !== 1 ? 's' : ''}
            </Text>
          </div>
        }
      >
        {tools.length === 0 ? (
          <Empty description="No tools available" image={Empty.PRESENTED_IMAGE_SIMPLE} />
        ) : (
          <div className="space-y-2">
            {tools.map(tool => (
              <div key={tool.name} className="flex items-start gap-2 p-2 rounded">
                <Checkbox
                  checked={isToolSelected(server.id, tool.name)}
                  onChange={() => handleToolToggle(server.id, tool.name)}
                  disabled={!selection}
                />
                <div className="flex-1">
                  <Text strong className="text-sm">{tool.name}</Text>
                  {tool.description && (
                    <div className="text-xs text-gray-500 mt-1">{tool.description}</div>
                  )}
                </div>
              </div>
            ))}
          </div>
        )}
      </Panel>
    )
  }

  return (
    <Modal
      title={
        <div className="flex items-center gap-2">
          <ToolOutlined />
          <span>Select MCP Tools</span>
        </div>
      }
      open={visible}
      onOk={onClose}
      onCancel={onClose}
      width={700}
      okText="Done"
      cancelButtonProps={{ style: { display: 'none' } }}
    >
      {enabledServers.length === 0 ? (
        <Empty
          description="No MCP servers available"
          image={Empty.PRESENTED_IMAGE_SIMPLE}
        />
      ) : (
        <Collapse accordion>
          {enabledServers.map(server => renderServerPanel(server))}
        </Collapse>
      )}
    </Modal>
  )
}
