import { useState } from 'react'
import { Alert, Button, Card, Typography } from 'antd'
import {
  ToolOutlined,
  CheckCircleOutlined,
  CloseCircleOutlined,
} from '@ant-design/icons'
import {
  createExtension,
  type ChatExtension,
  type ContentRendererProps,
} from '../../core/extensions'
import { Stores } from '@/core/stores'
import { createMcpStore, type McpToolCall } from './Mcp.store'
import type { MessageContentDataToolResult } from '@/api-client/types'
import { ToolCallPendingApprovalContent } from './components/ToolCallPendingApprovalContent'
import { McpServerSelector } from './components/McpServerSelector'

const { Text } = Typography

/**
 * MCP Tool Call UI Component
 * Shows approval UI when status is 'pending_approval'
 */
function McpToolCallUI({ toolCall }: { toolCall: McpToolCall }) {
  const [isExpanded, setIsExpanded] = useState(false)

  // Show approval UI for pending approval status
  if (toolCall.status === 'pending_approval') {
    return <ToolCallPendingApprovalContent toolCall={toolCall} />
  }

  const getStatusIcon = () => {
    switch (toolCall.status) {
      case 'started':
        return <ToolOutlined spin className="text-blue-500" />
      case 'completed':
        return <CheckCircleOutlined className="text-green-500" />
      case 'error':
        return <CloseCircleOutlined className="text-red-500" />
    }
  }

  const getStatusText = () => {
    switch (toolCall.status) {
      case 'started':
        return 'Running...'
      case 'completed':
        return 'Completed'
      case 'error':
        return 'Failed'
    }
  }

  return (
    <Card
      size="small"
      className="mb-2"
      style={{ backgroundColor: 'rgba(0, 0, 0, 0.02)' }}
    >
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-2">
          {getStatusIcon()}
          <Text strong>{toolCall.tool_name}</Text>
          <Text type="secondary" className="text-xs">
            ({toolCall.server})
          </Text>
          <Text type="secondary" className="text-xs">
            {getStatusText()}
          </Text>
        </div>
        <Button
          size="small"
          type="text"
          onClick={() => setIsExpanded(!isExpanded)}
        >
          {isExpanded ? 'Hide' : 'Show'} details
        </Button>
      </div>

      {isExpanded && (
        <div className="mt-2 text-xs">
          {toolCall.input !== undefined && (
            <div className="mb-2">
              <Text strong>Input:</Text>
              <pre className="bg-gray-100 p-2 rounded mt-1 overflow-auto max-h-40">
                {JSON.stringify(toolCall.input, null, 2)}
              </pre>
            </div>
          )}

          {toolCall.result !== undefined && (
            <div className="mb-2">
              <Text strong>Result:</Text>
              <pre className="bg-gray-100 p-2 rounded mt-1 overflow-auto max-h-40">
                {JSON.stringify(toolCall.result, null, 2)}
              </pre>
            </div>
          )}

          {toolCall.error && (
            <Alert
              type="error"
              message="Error"
              description={toolCall.error}
              showIcon
            />
          )}
        </div>
      )}
    </Card>
  )
}

/**
 * MCP tool result content renderer component
 * Renders tool execution results from MCP servers
 */
function McpToolResultRenderer({ content: data }: ContentRendererProps) {
  // Access store reactively in React component
  const mcpStore = Stores.Chat.McpStore

  // data is the full MessageContent object, data.content has the tool result data
  const toolResultData = data.content as MessageContentDataToolResult

  if (!toolResultData.tool_use_id) {
    return null
  }

  const toolCall = mcpStore.getToolCall(toolResultData.tool_use_id)

  if (!toolCall) {
    return null
  }

  return <McpToolCallUI toolCall={toolCall} />
}

/**
 * MCP active calls indicator component
 */
function McpActiveCallsIndicator() {
  // Access store reactively in React component
  const mcpStore = Stores.Chat.McpStore
  const activeCalls = mcpStore.getActiveCalls()

  if (activeCalls.length === 0) {
    return null
  }

  return (
    <div className="mb-4">
      <Alert
        type="info"
        message={`${activeCalls.length} tool call(s) in progress`}
        showIcon
        icon={<ToolOutlined spin />}
      />
    </div>
  )
}

/**
 * MCP Extension
 * Handles MCP tool calls, approval workflows, and renders tool call UI
 */
const mcpExtension: ChatExtension = createExtension({
  name: 'mcp',
  description: 'Handles MCP tool calls and approval workflows',
  priority: 50, // Higher priority to handle events early

  // Create independent extension store
  store: {
    name: 'McpStore',
    createStore: createMcpStore,
  },

  initialize: async () => {
    console.log('[MCP Extension] Initialized')
  },

  // Type-safe SSE event handlers
  sseEventHandlers: {
    mcpToolStart: async (data, _get, _set) => {
      // data is automatically typed as SSEChatStreamMcpToolStartData
      // Access store via __state to avoid triggering React hooks outside component context
      const mcpStore = Stores.Chat.__state.McpStore

      mcpStore.addToolCall({
        tool_use_id: data.tool_use_id,
        server: data.server,
        tool_name: data.tool_name,
        status: 'started',
      })

      console.log('[MCP Extension] Tool started:', data.tool_name)
    },

    mcpApprovalRequired: async (data, _get, _set) => {
      // data is automatically typed as SSEChatStreamMcpApprovalRequiredData
      const mcpStore = Stores.Chat.__state.McpStore

      mcpStore.updateToolCall(data.tool_use_id, {
        status: 'pending_approval',
        input: data.input,
      })

      console.log('[MCP Extension] Approval required for:', data.tool_name)
    },

    mcpToolComplete: async (data, _get, _set) => {
      // data is automatically typed as SSEChatStreamMcpToolCompleteData
      const mcpStore = Stores.Chat.__state.McpStore

      mcpStore.updateToolCall(data.tool_use_id, {
        status: data.is_error ? 'error' : 'completed',
        error: data.is_error ? 'Tool execution failed' : undefined,
      })

      console.log(
        '[MCP Extension] Tool completed:',
        data.tool_use_id,
        data.is_error ? '(error)' : '(success)',
      )
    },
  },

  // Compose request fields to include MCP config and approval decisions
  composeRequestFields: async () => {
    const { Stores } = await import('@/core/stores')
    const mcpStore = Stores.Chat.__state.McpStore
    const selectedServers = mcpStore.getSelectedServersConfig()
    const approvalDecisions = mcpStore.getApprovalDecisions()

    const fields: any = {}

    // Add MCP config if servers are selected
    if (selectedServers.length > 0) {
      fields.enable_mcp = true
      fields.mcp_config = { mcp_servers: selectedServers }
      console.log('[MCP Extension] Including MCP config:', fields.mcp_config)
    }

    // Add approval decisions if present
    if (approvalDecisions.length > 0) {
      fields.tool_approvals = approvalDecisions
      console.log('[MCP Extension] Including approval decisions:', approvalDecisions)
    }

    return fields
  },

  // Load conversation MCP settings when conversation is opened
  onConversationLoad: async (conversation) => {
    const { Stores } = await import('@/core/stores')
    const { ApiClient } = await import('@/api-client')
    const mcpStore = Stores.Chat.__state.McpStore

    // Set current conversation ID
    mcpStore.setCurrentConversation(conversation.id)

    try {
      // Load conversation MCP settings from backend
      const response = await ApiClient.Conversation.getMcpSettings({ id: conversation.id })

      // Get available servers to compute selectedServers from disabledServers
      // Access __state directly on the McpServer store (outside React context)
      const mcpServerState = Stores.McpServer.__state
      const availableServers = (mcpServerState?.servers || []).filter(s => s.enabled)
      const availableServerIds = new Set(availableServers.map(s => s.id))

      if (response.settings) {
        // Get disabled servers from backend
        const disabledServers = response.settings.disabled_servers || []
        const disabledServerIds = new Set(disabledServers.map(d => d.server_id))

        // Compute selectedServers: all available servers that are NOT disabled
        const selectedServers = new Map<string, { server_id: string; tools: string[] }>()
        for (const serverId of availableServerIds) {
          if (!disabledServerIds.has(serverId)) {
            // Server is not disabled, add to selected with all tools
            selectedServers.set(serverId, { server_id: serverId, tools: [] })
          }
        }

        const config = {
          selectedServers,
          disabledServers,
          approvalMode: response.settings.approval_mode as 'disabled' | 'auto_approve' | 'manual_approve',
          autoApprovedTools: response.settings.auto_approved_tools || [],
        }

        mcpStore.loadConversationConfig(conversation.id, config)
        console.log('[MCP Extension] Loaded conversation MCP config:', conversation.id, {
          availableServers: availableServerIds.size,
          disabledServers: disabledServers.length,
          selectedServers: selectedServers.size,
        })
      } else {
        // If settings don't exist yet, select all available servers by default
        const selectedServers = new Map<string, { server_id: string; tools: string[] }>()
        for (const serverId of availableServerIds) {
          selectedServers.set(serverId, { server_id: serverId, tools: [] })
        }

        const config = {
          selectedServers,
          disabledServers: [],
          approvalMode: 'manual_approve' as const,
          autoApprovedTools: [],
        }

        mcpStore.loadConversationConfig(conversation.id, config)
        console.log('[MCP Extension] No existing config, using defaults with all servers enabled:', conversation.id)
      }
    } catch (error) {
      // If settings don't exist yet, create default config with all servers enabled
      const mcpServerState = Stores.McpServer.__state
      const availableServers = (mcpServerState?.servers || []).filter(s => s.enabled)
      const selectedServers = new Map<string, { server_id: string; tools: string[] }>()
      for (const server of availableServers) {
        selectedServers.set(server.id, { server_id: server.id, tools: [] })
      }

      const config = {
        selectedServers,
        disabledServers: [],
        approvalMode: 'manual_approve' as const,
        autoApprovedTools: [],
      }

      mcpStore.loadConversationConfig(conversation.id, config)
      console.log('[MCP Extension] Error loading config, using defaults:', conversation.id, error)
    }
  },

  // Clear approval decisions after message is sent
  onMessageSent: async () => {
    const { Stores } = await import('@/core/stores')
    const mcpStore = Stores.Chat.__state.McpStore
    const chatStore = Stores.Chat.__state

    // Get current conversation from chat store
    const conversation = chatStore.conversation

    // Handle new conversation creation
    if (conversation?.id && !mcpStore.currentConversationId) {
      console.log('[MCP Extension] New conversation created, transferring pending config:', conversation.id)

      // Transfer pending config to the new conversation
      mcpStore.transferPendingConfig(conversation.id)

      // Set current conversation ID
      mcpStore.setCurrentConversation(conversation.id)

      // Get available server IDs for proper disabled_servers computation
      const mcpServerState = Stores.McpServer.__state
      const availableServerIds = (mcpServerState?.servers || [])
        .filter(s => s.enabled)
        .map(s => s.id)

      // Save settings to backend with available server IDs
      try {
        await mcpStore.saveConversationConfig(conversation.id, availableServerIds)
      } catch (error) {
        console.error('[MCP Extension] Failed to save config for new conversation:', error)
      }
    }

    mcpStore.clearApprovalDecisions()
    console.log('[MCP Extension] Cleared approval decisions after message sent')

    return {}
  },

  // Register content type components
  contentTypes: {
    tool_result: McpToolResultRenderer,
  },

  // Register slot components
  slots: {
    toolbar_actions: { component: McpServerSelector, order: 20 },
    message_list_header: { component: McpActiveCallsIndicator, order: 50 },
  },

  cleanup: async () => {
    console.log('[MCP Extension] Cleaned up')
  },
})

export default mcpExtension
