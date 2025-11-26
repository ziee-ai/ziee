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
  type SSEEvent,
  type HandleSSEEventResult,
  type ContentRendererProps,
  type SSEEventData,
} from '../../core/extensions'
import { Stores } from '@/core/stores'
import { createMcpStore, type McpToolCall } from './McpStore.store'

const { Text } = Typography

/**
 * MCP Tool Call UI Component
 */
function McpToolCallUI({ toolCall }: { toolCall: McpToolCall }) {
  const [isExpanded, setIsExpanded] = useState(false)

  const getStatusIcon = () => {
    switch (toolCall.status) {
      case 'started':
        return <ToolOutlined spin className="text-blue-500" />
      case 'pending_approval':
        return <ToolOutlined className="text-orange-500" />
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
      case 'pending_approval':
        return 'Awaiting approval'
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
 * MCP tool call content renderer component
 */
function McpToolCallRenderer({ content }: ContentRendererProps) {
  // Access store reactively in React component
  const mcpStore = Stores.Chat.McpStore

  const toolCallData = content.content as { tool_use_id?: string }

  if (!toolCallData.tool_use_id) {
    return null
  }

  const toolCall = mcpStore.getToolCall(toolCallData.tool_use_id)

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
  name: 'McpStore',
  description: 'Handles MCP tool calls and approval workflows',
  priority: 50, // Higher priority to handle events early

  // Create independent extension store
  createStore: createMcpStore,

  initialize: async () => {
    console.log('[MCP Extension] Initialized')
  },

  handleSSEEvent: async (event: SSEEvent): Promise<HandleSSEEventResult> => {
    // Access store via __state to avoid triggering React hooks outside component context
    const mcpStore = Stores.Chat.__state.McpStore

    // Handle MCP tool start
    if (event.event_type === 'mcpToolStart') {
      const data = event.data as SSEEventData<'mcpToolStart'>
      mcpStore.addToolCall({
        tool_use_id: data.tool_use_id,
        server: data.server,
        tool_name: data.tool_name,
        status: 'started',
      })
      console.log('[MCP Extension] Tool started:', data.tool_name)
      return { handled: true, uiUpdates: [] }
    }

    // Handle MCP approval required
    if (event.event_type === 'mcpApprovalRequired') {
      const data = event.data as SSEEventData<'mcpApprovalRequired'>
      mcpStore.updateToolCall(data.tool_use_id, {
        status: 'pending_approval',
        input: data.input,
      })
      console.log('[MCP Extension] Approval required for:', data.tool_name)
      // TODO: Show approval UI
      return { handled: true, uiUpdates: [] }
    }

    // Handle MCP tool complete
    if (event.event_type === 'mcpToolComplete') {
      const data = event.data as SSEEventData<'mcpToolComplete'>
      mcpStore.updateToolCall(data.tool_use_id, {
        status: data.is_error ? 'error' : 'completed',
        error: data.is_error ? 'Tool execution failed' : undefined,
      })
      console.log(
        '[MCP Extension] Tool completed:',
        data.tool_use_id,
        data.is_error ? '(error)' : '(success)',
      )
      return { handled: true, uiUpdates: [] }
    }

    return { handled: false }
  },

  // Register content type components
  contentTypes: {
    mcp_tool_call: McpToolCallRenderer,
  },

  // Register slot components
  slots: {
    message_list_header: { component: McpActiveCallsIndicator, order: 50 },
  },

  cleanup: async () => {
    console.log('[MCP Extension] Cleaned up')
  },
})

export default mcpExtension
