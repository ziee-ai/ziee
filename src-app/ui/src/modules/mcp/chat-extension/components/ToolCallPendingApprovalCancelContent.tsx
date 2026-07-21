import { Alert, Text } from '@ziee/kit'
import type { McpToolCall } from '@/modules/mcp/stores/mcpComposer'
import { TOOL_STATUS } from '@/modules/chat/core/tool-status'
import { mcpServerParenLabel } from '@/modules/mcp/chat-extension/serverLabel'

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
  // A cancel is a user choice, not a failure — render it NEUTRAL (the shared
  // `cancelled` status: a slashed circle in muted gray), never the red X / error
  // tone reserved for a genuinely failed tool call (finding #2).
  const CancelIcon = TOOL_STATUS.cancelled.icon
  const serverLabel = mcpServerParenLabel(toolCall.server)
  return (
    <div className="my-2">
      <Alert
        tone="neutral"
        data-testid="mcp-tool-approval-cancel-alert"
        icon={<CancelIcon className={TOOL_STATUS.cancelled.color} />}
        title={
          <div className="flex items-center gap-2 min-w-0">
            <Text strong className="truncate">Tool Call Cancelled: {toolCall.tool_name}</Text>
            {serverLabel && (
              <Text type="secondary" className="text-xs whitespace-nowrap">
                {serverLabel}
              </Text>
            )}
          </div>
        }
        description={
          toolCall.error || 'Tool execution was denied or cancelled by the user'
        }
      />
    </div>
  )
}
