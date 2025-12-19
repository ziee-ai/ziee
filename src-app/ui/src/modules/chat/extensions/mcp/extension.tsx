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
} from '@/modules/chat/core/extensions'
import { Stores } from '@/core/stores'
import { createMcpStore, type McpToolCall } from '@/modules/chat/extensions/mcp/Mcp.store'
import type { MessageContent, MessageContentDataToolResult, MessageContentDataToolUse, MessageWithContent } from '@/api-client/types'
import { ToolCallPendingApprovalContent } from '@/modules/chat/extensions/mcp/components/ToolCallPendingApprovalContent'
import { McpServerSelector } from '@/modules/chat/extensions/mcp/components/McpServerSelector'

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
              <pre className="p-2 rounded mt-1 overflow-auto max-h-40">
                {JSON.stringify(toolCall.input, null, 2)}
              </pre>
            </div>
          )}

          {toolCall.result !== undefined && (
            <div className="mb-2">
              <Text strong>Result:</Text>
              <pre className="p-2 rounded mt-1 overflow-auto max-h-40">
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
 * MCP tool use content renderer component
 * Renders tool calls from MCP servers (the call itself, before result)
 */
function McpToolUseRenderer({ content: data }: ContentRendererProps) {
  const [isExpanded, setIsExpanded] = useState(false)
  // Access toolCalls Map directly to create a reactive subscription
  // Using getToolCall() method doesn't trigger re-renders when store updates
  const { toolCalls } = Stores.Chat.McpStore
  const { servers } = Stores.McpServer
  const toolUseData = data.content as MessageContentDataToolUse

  if (!toolUseData.id) {
    return null
  }

  const toolCall = toolCalls.get(toolUseData.id)

  // If we have a tracked tool call, render it
  if (toolCall) {
    return <McpToolCallUI toolCall={toolCall} />
  }

  // Look up server name from server_id
  const server = servers.find(s => s.id === toolUseData.server_id)
  const serverName = server?.display_name || toolUseData.server_id || 'Unknown'

  // Otherwise render a basic view for untracked tool calls (e.g., from history)
  return (
    <Card size="small" className="mb-2" style={{ backgroundColor: 'rgba(0, 0, 0, 0.02)' }}>
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-2">
          <ToolOutlined className="text-blue-500" />
          <Text strong>{toolUseData.name || 'Tool Call'}</Text>
          <Text type="secondary" className="text-xs">({serverName})</Text>
        </div>
        {toolUseData.input && (
          <Button size="small" type="text" onClick={() => setIsExpanded(!isExpanded)}>
            {isExpanded ? 'Hide' : 'Show'} parameters
          </Button>
        )}
      </div>
      {isExpanded && toolUseData.input && (
        <pre className=" p-2 rounded mt-2 overflow-auto max-h-40 text-xs">
          {JSON.stringify(toolUseData.input, null, 2)}
        </pre>
      )}
    </Card>
  )
}

/**
 * MCP tool result content renderer component
 * Renders tool execution results from MCP servers
 */
function McpToolResultRenderer({ content: data }: ContentRendererProps) {
  // Access toolCalls Map directly to create a reactive subscription
  const { toolCalls } = Stores.Chat.McpStore

  // data is the full MessageContent object, data.content has the tool result data
  const toolResultData = data.content as MessageContentDataToolResult

  if (!toolResultData.tool_use_id) {
    return null
  }

  const toolCall = toolCalls.get(toolResultData.tool_use_id)

  if (!toolCall) {
    return null
  }

  return <McpToolCallUI toolCall={toolCall} />
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
    mcpToolStart: async (data, get, set) => {
      // data is automatically typed as SSEChatStreamMcpToolStartData
      // Access store via __state to avoid triggering React hooks outside component context
      const mcpStore = Stores.Chat.__state.McpStore

      mcpStore.addToolCall({
        tool_use_id: data.tool_use_id,
        server: data.server,
        tool_name: data.tool_name,
        status: 'started',
      })

      // Inject tool_use content block into streaming message so McpToolUseRenderer can mount
      // This ensures tool calls are visible during auto-approve execution
      const chatState = get()
      let streamingMessage = chatState.streamingMessage
      const now = new Date().toISOString()

      // Create tool_use content block
      const toolUseContent: MessageContent = {
        id: '',
        message_id: '',
        content_type: 'tool_use',
        content: {
          type: 'tool_use',
          id: data.tool_use_id,
          name: data.tool_name,
          server_id: data.server,
        } as MessageContentDataToolUse,
        sequence_order: 0,
        created_at: now,
        updated_at: now,
      }

      if (streamingMessage) {
        // Check if this tool_use content already exists (avoid duplicates)
        const exists = streamingMessage.contents.some(
          c => c.content_type === 'tool_use' &&
               (c.content as MessageContentDataToolUse).id === data.tool_use_id
        )
        if (!exists) {
          toolUseContent.id = `${streamingMessage.id}-tool-${data.tool_use_id}`
          toolUseContent.message_id = streamingMessage.id
          toolUseContent.sequence_order = streamingMessage.contents.length

          const updatedMessage = {
            ...streamingMessage,
            contents: [...streamingMessage.contents, toolUseContent],
          }

          const newMessages = new Map(chatState.messages)
          newMessages.set(updatedMessage.id, updatedMessage)
          set({
            streamingMessage: updatedMessage,
            messages: newMessages,
          })
        }
      } else {
        // No streaming message exists - CREATE one with the tool_use block
        const messageId = `streaming-${Date.now()}`
        toolUseContent.id = `${messageId}-tool-${data.tool_use_id}`
        toolUseContent.message_id = messageId

        const newMessage: MessageWithContent = {
          id: messageId,
          role: 'assistant',
          contents: [toolUseContent],
          originated_from_id: '',
          edit_count: 0,
          created_at: now,
        }

        const newMessages = new Map(chatState.messages)
        newMessages.set(newMessage.id, newMessage)
        set({
          streamingMessage: newMessage,
          messages: newMessages,
        })
      }

      console.log('[MCP Extension] Tool started:', data.tool_name)
    },

    mcpApprovalRequired: async (data, get, set) => {
      // data is automatically typed as SSEChatStreamMcpApprovalRequiredData
      const mcpStore = Stores.Chat.__state.McpStore

      // Use addToolCall instead of updateToolCall - the tool call doesn't exist yet
      // because mcpToolStart is NOT sent when approval is required
      mcpStore.addToolCall({
        tool_use_id: data.tool_use_id,
        server: data.server,
        server_id: data.server_id,
        tool_name: data.tool_name,
        status: 'pending_approval',
        input: data.input,
      })

      // Inject tool_use content block into streaming message so McpToolUseRenderer can mount
      // Without this, the approval UI won't show because there's no content block to render
      const chatState = get()
      let streamingMessage = chatState.streamingMessage
      const now = new Date().toISOString()

      // Create tool_use content block
      const toolUseContent: MessageContent = {
        id: '', // Will be set below based on message id
        message_id: '', // Will be set below
        content_type: 'tool_use',
        content: {
          type: 'tool_use',
          id: data.tool_use_id,
          name: data.tool_name,
          server_id: data.server_id,
          input: data.input,
        } as MessageContentDataToolUse,
        sequence_order: 0,
        created_at: now,
        updated_at: now,
      }

      if (streamingMessage) {
        // Streaming message exists - add tool_use content to it
        toolUseContent.id = `${streamingMessage.id}-tool-${data.tool_use_id}`
        toolUseContent.message_id = streamingMessage.id
        toolUseContent.sequence_order = streamingMessage.contents.length

        const updatedMessage = {
          ...streamingMessage,
          contents: [...streamingMessage.contents, toolUseContent],
        }

        const newMessages = new Map(chatState.messages)
        newMessages.set(updatedMessage.id, updatedMessage)
        set({
          streamingMessage: updatedMessage,
          messages: newMessages,
        })

        console.log('[MCP Extension] Added tool_use content block to existing streaming message:', data.tool_name)
      } else {
        // No streaming message exists - CREATE one with the tool_use block
        // This happens when LLM returns a tool call without any text first
        const messageId = `streaming-${Date.now()}`
        toolUseContent.id = `${messageId}-tool-${data.tool_use_id}`
        toolUseContent.message_id = messageId

        const newMessage: MessageWithContent = {
          id: messageId,
          role: 'assistant',
          contents: [toolUseContent],
          originated_from_id: '',
          edit_count: 0,
          created_at: now,
        }

        const newMessages = new Map(chatState.messages)
        newMessages.set(newMessage.id, newMessage)
        set({
          streamingMessage: newMessage,
          messages: newMessages,
        })

        console.log('[MCP Extension] Created new streaming message with tool_use block:', data.tool_name)
      }

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

  // Allow empty text when there are pending tool approvals
  beforeSendMessage: async () => {
    const { Stores } = await import('@/core/stores')
    const mcpStore = Stores.Chat.__state.McpStore

    // Check if there are approval decisions queued to send
    const approvalDecisions = mcpStore.getApprovalDecisions()
    const hasApprovalDecisions = approvalDecisions.length > 0

    if (hasApprovalDecisions) {
      // Discard text extension's cancel since we're sending tool approvals
      console.log('[MCP Extension] Has approval decisions, discarding text cancel')
      return { cancel: false, discardCancel: ['text'] }
    }

    return { cancel: false }
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
          loopSettings: response.settings.loop_settings,
        }

        mcpStore.loadConversationConfig(conversation.id, config)
        console.log('[MCP Extension] Loaded conversation MCP config:', conversation.id, {
          availableServers: availableServerIds.size,
          disabledServers: disabledServers.length,
          selectedServers: selectedServers.size,
          loopSettings: response.settings.loop_settings,
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
          loopSettings: undefined,  // Use defaults
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
        loopSettings: undefined,  // Use defaults
      }

      mcpStore.loadConversationConfig(conversation.id, config)
      console.log('[MCP Extension] Error loading config, using defaults:', conversation.id, error)
    }

    // Load pending approvals for the current branch (to restore state after page refresh)
    if (conversation.active_branch_id) {
      try {
        const approvalsResponse = await ApiClient.Branch.getPendingApprovals({
          branch_id: conversation.active_branch_id,
        })

        if (approvalsResponse.approvals && approvalsResponse.approvals.length > 0) {
          for (const approval of approvalsResponse.approvals) {
            mcpStore.addToolCall({
              tool_use_id: approval.tool_use_id,
              server: approval.server_name,
              server_id: approval.server_id,
              tool_name: approval.tool_name,
              status: 'pending_approval',
              input: approval.input,
            })
          }
          console.log(
            '[MCP Extension] Loaded pending approvals:',
            approvalsResponse.approvals.length,
          )
        }
      } catch (error) {
        console.error('[MCP Extension] Failed to load pending approvals:', error)
      }
    }
  },

  // Clear approval decisions after message is sent
  onMessageSent: async () => {
    const { Stores } = await import('@/core/stores')
    // Use __state on McpStore too since it's also a proxy
    const mcpStore = Stores.Chat.__state.McpStore.__state
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
    tool_use: McpToolUseRenderer,
    tool_result: McpToolResultRenderer,
  },

  // Register slot components
  slots: {
    toolbar_actions: { component: McpServerSelector, order: 20 },
  },

  cleanup: async () => {
    console.log('[MCP Extension] Cleaned up')
  },
})

export default mcpExtension
