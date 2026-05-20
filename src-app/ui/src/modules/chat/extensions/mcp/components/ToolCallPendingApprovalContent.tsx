import { useState } from 'react'
import { Alert, Button, Space, Typography } from 'antd'
import {
  ClockCircleOutlined,
  CheckOutlined,
  CloseOutlined,
} from '@ant-design/icons'
import { Stores } from '@/core/stores'
import type { McpToolCall } from '@/modules/chat/extensions/mcp/Mcp.store'

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
  const [isSubmitting, setIsSubmitting] = useState(false)

  const handleApproveOnce = async () => {
    setIsSubmitting(true)
    const mcpStore = Stores.Chat.__state.McpStore
    // Optimistic status update: immediately switch out of 'pending_approval' so the
    // approval panel disappears and shows the 'Running…' card instead. This update
    // lives in McpStore (not local React state) and therefore survives the component
    // remount that happens when loadMessages replaces the messages Map after streaming.
    mcpStore.updateToolCall(toolCall.tool_use_id, { status: 'started' })
    try {
      mcpStore.addApprovalDecision({
        tool_use_id: toolCall.tool_use_id,
        decision: 'approve',
        note: 'User approved tool execution (once)',
      })
      await Stores.Chat.sendMessage()
      console.log('[MCP Approval] Tool approved once:', toolCall.tool_name)
    } catch (error) {
      console.error('[MCP Approval] Failed to approve tool:', error)
      // Revert optimistic update so the approval panel reappears on failure
      mcpStore.updateToolCall(toolCall.tool_use_id, { status: 'pending_approval' })
      setIsSubmitting(false)
    }
  }

  const handleApproveForConversation = async () => {
    setIsSubmitting(true)
    const mcpStore = Stores.Chat.__state.McpStore
    // Optimistic status update (same rationale as handleApproveOnce)
    mcpStore.updateToolCall(toolCall.tool_use_id, { status: 'started' })
    try {
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
          await mcpStore.saveConversationConfig(conversationId, availableServerIds, undefined, true)
        }
      }

      // 3. Approve current tool call
      mcpStore.addApprovalDecision({
        tool_use_id: toolCall.tool_use_id,
        decision: 'approve',
        note: 'User approved tool for this conversation',
      })

      await Stores.Chat.sendMessage()
      console.log('[MCP Approval] Tool approved for conversation:', toolCall.tool_name)
    } catch (error) {
      console.error('[MCP Approval] Failed to approve tool:', error)
      mcpStore.updateToolCall(toolCall.tool_use_id, { status: 'pending_approval' })
      setIsSubmitting(false)
    }
  }

  const handleDeny = async () => {
    setIsSubmitting(true)
    const mcpStore = Stores.Chat.__state.McpStore
    // Optimistic status update to 'error' so the panel immediately shows denied state
    mcpStore.updateToolCall(toolCall.tool_use_id, {
      status: 'error',
      error: 'Tool execution denied by user',
    })
    try {
      mcpStore.addApprovalDecision({
        tool_use_id: toolCall.tool_use_id,
        decision: 'deny',
        note: 'User denied tool execution',
      })
      await Stores.Chat.sendMessage()
      console.log('[MCP Approval] Tool denied:', toolCall.tool_name)
    } catch (error) {
      console.error('[MCP Approval] Failed to deny tool:', error)
      mcpStore.updateToolCall(toolCall.tool_use_id, { status: 'pending_approval' })
      setIsSubmitting(false)
    }
  }

  return (
    <div className="my-2" data-testid={`tool-approval-${toolCall.tool_use_id}`}>
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
                  data-testid="tool-approval-approve-once"
                >
                  Approve once
                </Button>
                <Button
                  icon={<CheckOutlined />}
                  onClick={handleApproveForConversation}
                  loading={isSubmitting}
                  size="small"
                  data-testid="tool-approval-approve-conv"
                >
                  Approve for this conversation
                </Button>
                <Button
                  danger
                  icon={<CloseOutlined />}
                  onClick={handleDeny}
                  loading={isSubmitting}
                  size="small"
                  data-testid="tool-approval-deny"
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
