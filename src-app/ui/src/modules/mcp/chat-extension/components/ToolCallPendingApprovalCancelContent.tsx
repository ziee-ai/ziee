import { Alert, Text } from '@/components/ui'
import { Ban } from 'lucide-react'
import type { McpToolCall } from '@/modules/mcp/stores/McpComposer.store'

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
        tone="error"
        data-testid="mcp-tool-approval-cancel-alert"
        icon={<Ban />}
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
      />
    </div>
  )
}
