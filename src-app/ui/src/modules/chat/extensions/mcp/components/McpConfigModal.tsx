import { useEffect, useState } from 'react'
import { Modal, Collapse, Switch, Tag, Typography, Empty, Checkbox, Select, Divider, Button, Space } from 'antd'
import type { CollapseProps } from 'antd'
import { ToolOutlined } from '@ant-design/icons'
import { Stores } from '@/core/stores'
import { ApiClient } from '@/api-client'
import type { Tool } from '@/api-client/types'
import { PENDING_CONVERSATION_KEY } from '../Mcp.store'

const { Text, Title } = Typography

interface McpConfigModalProps {
  visible: boolean
  onClose: () => void
}

/**
 * MCP Configuration Modal
 *
 * Unified modal for managing MCP settings for the current conversation:
 * - Server and tool selection
 * - Approval mode (disabled/auto_approve/manual_approve)
 * - Auto-approved tools list
 *
 * Features:
 * - Per-conversation configuration (persisted to backend)
 * - Works for both new and existing conversations
 * - Groups tools by MCP server
 * - Server-level toggle (enable all tools from server)
 * - Individual tool toggles
 */
export function McpConfigModal({
  visible,
  onClose,
}: McpConfigModalProps) {
  const { servers } = Stores.McpServer  // Reactive access to MCP module store
  const mcpStore = Stores.Chat.McpStore
  // Extract all store properties unconditionally at the top (store proxy uses hooks)
  const { selectedServers, currentConversationId, conversationConfigs } = mcpStore

  // Local state for tools (loaded on demand)
  const [serverTools, setServerTools] = useState<Map<string, Tool[]>>(new Map())
  const [loadingTools, setLoadingTools] = useState<Set<string>>(new Set())
  const [saving, setSaving] = useState(false)
  const [savingDefaults, setSavingDefaults] = useState(false)

  // Get enabled servers (available for selection)
  const enabledServers = servers.filter(s => s.enabled)

  // Get current conversation config (or pending config for new conversations)
  const configKey = currentConversationId || PENDING_CONVERSATION_KEY
  const conversationConfig = conversationConfigs.get(configKey)
  const approvalMode = conversationConfig?.approvalMode || 'manual_approve'

  // Lazy load tools when modal opens
  useEffect(() => {
    if (visible) {
      enabledServers.forEach(async server => {
        if (!serverTools.has(server.id) && !loadingTools.has(server.id)) {
          setLoadingTools(prev => new Set(prev).add(server.id))

          try {
            const response = await ApiClient.McpServerRuntime.listTools({ id: server.id })
            setServerTools(prev => new Map(prev).set(server.id, response.tools))
          } catch (error) {
            console.error('[MCP Config Modal] Failed to load tools for server:', server.id, error)
          } finally {
            setLoadingTools(prev => {
              const next = new Set(prev)
              next.delete(server.id)
              return next
            })
          }
        }
      })
    }
  }, [visible, enabledServers.length])

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

  // Handle approval mode change (works for both existing and new conversations)
  const handleApprovalModeChange = (value: 'disabled' | 'auto_approve' | 'manual_approve') => {
    mcpStore.setApprovalMode(currentConversationId, value)
  }

  // Handle save (only for existing conversations - new ones save on first message)
  const handleSave = async () => {
    if (!currentConversationId) {
      // For new conversations, settings are stored in pending config
      // They will be saved when the conversation is created
      console.log('[MCP Config Modal] Settings stored in pending config (will save on first message)')
      return
    }

    setSaving(true)
    try {
      // Pass available server IDs to compute disabled_servers
      const availableServerIds = enabledServers.map(s => s.id)
      await mcpStore.saveConversationConfig(currentConversationId, availableServerIds)
      console.log('[MCP Config Modal] Configuration saved successfully')
    } catch (error) {
      console.error('[MCP Config Modal] Failed to save configuration:', error)
    } finally {
      setSaving(false)
    }
  }

  // Handle modal close - auto-save if conversation exists
  const handleClose = async () => {
    if (currentConversationId) {
      await handleSave()
    }
    onClose()
  }

  // Handle save as default
  const handleSaveAsDefault = async () => {
    setSavingDefaults(true)
    try {
      const availableServerIds = enabledServers.map(s => s.id)
      await mcpStore.saveUserDefaults(currentConversationId, availableServerIds)
      console.log('[MCP Config Modal] Saved as user defaults')
    } catch (error) {
      console.error('[MCP Config Modal] Failed to save as defaults:', error)
    } finally {
      setSavingDefaults(false)
    }
  }

  // Check if a tool is auto-approved
  const isToolAutoApproved = (serverId: string, toolName: string): boolean => {
    const autoApprovedTools = conversationConfig?.autoApprovedTools || []
    const serverEntry = autoApprovedTools.find(s => s.server_id === serverId)
    return serverEntry ? serverEntry.tools.includes(toolName) : false
  }

  // Handle auto-approve toggle (works for both existing and new conversations)
  const handleAutoApproveToggle = (serverId: string, toolName: string) => {
    mcpStore.toggleAutoApprovedTool(currentConversationId, serverId, toolName)
  }

  // Build collapse items
  const collapseItems: CollapseProps['items'] = enabledServers.map(server => {
    const tools = serverTools.get(server.id) || []
    const selection = selectedServers.get(server.id)

    return {
      key: server.id,
      label: (
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
      ),
      children: tools.length === 0 ? (
        <Empty description="No tools available" image={Empty.PRESENTED_IMAGE_SIMPLE} />
      ) : (
        <div className="space-y-2">
          {tools.map(tool => (
            <div key={tool.name} className="flex items-start gap-2 p-2 hover:bg-gray-50 dark:hover:bg-gray-800 rounded">
              <Checkbox
                checked={isToolSelected(server.id, tool.name)}
                onChange={() => handleToolToggle(server.id, tool.name)}
                disabled={!selection}
              />
              <div className="flex-1">
                <div className="flex items-center justify-between">
                  <Text strong className="text-sm">{tool.name}</Text>
                  {approvalMode === 'manual_approve' && (
                    <div className="flex items-center gap-1">
                      <Text type="secondary" className="text-xs">Auto</Text>
                      <Switch
                        size="small"
                        checked={isToolAutoApproved(server.id, tool.name)}
                        onChange={() => handleAutoApproveToggle(server.id, tool.name)}
                        disabled={!selection}
                      />
                    </div>
                  )}
                </div>
                {tool.description && (
                  <div className="text-xs text-gray-500 mt-1">{tool.description}</div>
                )}
              </div>
            </div>
          ))}
        </div>
      ),
    }
  })

  return (
    <Modal
      title={
        <div className="flex items-center gap-2">
          <ToolOutlined />
          <span>MCP Configuration</span>
        </div>
      }
      open={visible}
      onCancel={handleClose}
      width={800}
      footer={
        <Space>
          <Button onClick={handleSaveAsDefault} loading={savingDefaults}>
            Save as Default
          </Button>
          <Button type="primary" onClick={handleClose} loading={saving}>
            {currentConversationId ? 'Save & Close' : 'Close'}
          </Button>
        </Space>
      }
    >
      <div className="space-y-4">
        {/* Approval Mode Section */}
        <div>
          <Title level={5}>Approval Mode</Title>
          <Select
            value={approvalMode}
            onChange={handleApprovalModeChange}
            style={{ width: '100%' }}
            options={[
              {
                value: 'disabled',
                label: 'Disabled - MCP tools blocked',
              },
              {
                value: 'auto_approve',
                label: 'Auto Approve - Automatically approve all tools',
              },
              {
                value: 'manual_approve',
                label: 'Manual Approve - Require approval for each tool',
              },
            ]}
          />
        </div>

        <Divider />

        {/* Server & Tool Selection Section */}
        <div>
          <Title level={5}>Server & Tool Selection</Title>
          {enabledServers.length === 0 ? (
            <Empty
              description="No MCP servers available"
              image={Empty.PRESENTED_IMAGE_SIMPLE}
            />
          ) : (
            <Collapse accordion items={collapseItems} />
          )}
        </div>
      </div>
    </Modal>
  )
}
