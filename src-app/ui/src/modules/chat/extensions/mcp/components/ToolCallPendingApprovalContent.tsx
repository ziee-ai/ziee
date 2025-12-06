import { useState } from 'react'
import { Alert, Button, Space, Typography } from 'antd'
import {
  ClockCircleOutlined,
  CheckOutlined,
  CloseOutlined,
} from '@ant-design/icons'
import { Stores } from '@/core/stores'
import type { McpToolCall } from '../Mcp.store'

const { Text } = Typography

interface ToolCallPendingApprovalContentProps {
  toolCall: McpToolCall
}

/**
 * ToolCallPendingApprovalContent
 *
 * Renders inline approval UI for MCP tool calls requiring approval.
 * Following the reference pattern: inline content block with approve/deny buttons.
 *
 * User actions:
 * - Approve Once: Creates immediate approval, resumes chat
 * - Deny: Denies tool execution, cancels the tool call
 */
export function ToolCallPendingApprovalContent({
  toolCall,
}: ToolCallPendingApprovalContentProps) {
  const [isHidden, setIsHidden] = useState(false)
  const [isSubmitting, setIsSubmitting] = useState(false)

  // Hide the approval UI if already handled
  if (isHidden) {
    return null
  }

  const handleApproveOnce = async () => {
    setIsSubmitting(true)
    try {
      const mcpStore = Stores.Chat.__state.McpStore

      // Store approval decision (will be picked up by composeRequestFields)
      mcpStore.addApprovalDecision({
        tool_use_id: toolCall.tool_use_id,
        decision: 'approve',
        note: 'User approved tool execution (once)',
      })

      // Hide the approval UI
      setIsHidden(true)

      // Resume chat by sending empty message with approval decision
      // The MCP extension's composeRequestFields will include tool_approvals
      await Stores.Chat.sendMessage()

      console.log(
        '[MCP Approval] Tool approved once:',
        toolCall.tool_name,
      )
    } catch (error) {
      console.error('[MCP Approval] Failed to approve tool:', error)
      setIsSubmitting(false)
      setIsHidden(false) // Show UI again on error
    }
  }

  const handleApproveForConversation = async () => {
    setIsSubmitting(true)
    try {
      const mcpStore = Stores.Chat.__state.McpStore
      const chatState = Stores.Chat.__state
      const conversationId = chatState.conversation?.id || null

      // 1. Add tool to auto_approved_tools for this conversation
      if (toolCall.server_id) {
        mcpStore.toggleAutoApprovedTool(conversationId, toolCall.server_id, toolCall.tool_name)

        // 2. Persist to backend if conversation exists
        if (conversationId) {
          const mcpServerState = Stores.McpServer.__state
          const availableServerIds = (mcpServerState?.servers || [])
            .filter((s: { enabled: boolean }) => s.enabled)
            .map((s: { id: string }) => s.id)
          await mcpStore.saveConversationConfig(conversationId, availableServerIds)
        }
      }

      // 3. Approve current tool call
      mcpStore.addApprovalDecision({
        tool_use_id: toolCall.tool_use_id,
        decision: 'approve',
        note: 'User approved tool for this conversation',
      })

      setIsHidden(true)
      await Stores.Chat.sendMessage()

      console.log(
        '[MCP Approval] Tool approved for conversation:',
        toolCall.tool_name,
      )
    } catch (error) {
      console.error('[MCP Approval] Failed to approve tool:', error)
      setIsSubmitting(false)
      setIsHidden(false)
    }
  }

  const handleDeny = async () => {
    setIsSubmitting(true)
    try {
      const mcpStore = Stores.Chat.__state.McpStore

      // Store denial decision (will be picked up by composeRequestFields)
      mcpStore.addApprovalDecision({
        tool_use_id: toolCall.tool_use_id,
        decision: 'deny',
        note: 'User denied tool execution',
      })

      // Hide the approval UI
      setIsHidden(true)

      // Update tool call status in store
      mcpStore.updateToolCall(toolCall.tool_use_id, {
        status: 'error',
        error: 'Tool execution denied by user',
      })

      // Resume chat by sending empty message with denial decision
      await Stores.Chat.sendMessage()

      console.log('[MCP Approval] Tool denied:', toolCall.tool_name)
    } catch (error) {
      console.error('[MCP Approval] Failed to deny tool:', error)
      setIsSubmitting(false)
      setIsHidden(false) // Show UI again on error
    }
  }

  return (
    <div className="my-2">
      <Alert
        type="warning"
        icon={<ClockCircleOutlined />}
        title={
          <div>
            <Text strong>Tool Approval Required: {toolCall.tool_name}</Text>
            <Text type="secondary" className="ml-2 text-xs">
              ({toolCall.server})
            </Text>
          </div>
        }
        description={
          <div className="mt-2">
            <Text className="text-sm">
              This tool requires your approval before execution.
            </Text>

            {toolCall.input !== undefined && (
              <div className="mt-2">
                <Text strong className="text-xs">
                  Arguments:
                </Text>
                <pre className="p-2 rounded mt-1 overflow-auto max-h-40 text-xs">
                  {JSON.stringify(toolCall.input, null, 2)}
                </pre>
              </div>
            )}

            <div className="mt-3">
              <Space>
                <Button
                  type="primary"
                  icon={<CheckOutlined />}
                  onClick={handleApproveOnce}
                  loading={isSubmitting}
                  size="small"
                >
                  Approve once
                </Button>
                <Button
                  icon={<CheckOutlined />}
                  onClick={handleApproveForConversation}
                  loading={isSubmitting}
                  size="small"
                >
                  Approve for this conversation
                </Button>
                <Button
                  danger
                  icon={<CloseOutlined />}
                  onClick={handleDeny}
                  loading={isSubmitting}
                  size="small"
                >
                  Deny
                </Button>
              </Space>
            </div>
          </div>
        }
        showIcon
      />
    </div>
  )
}
