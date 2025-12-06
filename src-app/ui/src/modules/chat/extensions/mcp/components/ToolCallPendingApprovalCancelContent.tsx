import { Alert, Typography } from 'antd'
import { StopOutlined } from '@ant-design/icons'
import type { McpToolCall } from '../Mcp.store'

const { Text } = Typography

interface ToolCallPendingApprovalCancelContentProps {
  toolCall: McpToolCall
}

/**
 * ToolCallPendingApprovalCancelContent
 *
 * Renders cancellation message when a tool call approval is denied.
 * Purely informational - no user interactions.
 */
export function ToolCallPendingApprovalCancelContent({
  toolCall,
}: ToolCallPendingApprovalCancelContentProps) {
  return (
    <div className="my-2">
      <Alert
        type="error"
        icon={<StopOutlined />}
        title={
          <div>
            <Text strong>Tool Call Cancelled: {toolCall.tool_name}</Text>
            <Text type="secondary" className="ml-2 text-xs">
              ({toolCall.server})
            </Text>
          </div>
        }
        description={
          toolCall.error || 'Tool execution was denied or cancelled by the user'
        }
        showIcon
      />
    </div>
  )
}
