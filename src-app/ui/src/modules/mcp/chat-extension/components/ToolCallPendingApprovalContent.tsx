import { useState } from 'react'
import { Button, Card, Text } from '@/components/ui'
import { Clock, Check, X } from 'lucide-react'
import { Stores } from '@/core/stores'
import type { McpToolCall } from '@/modules/mcp/stores/McpComposer.store'
import { mcpServerParenLabel } from '@/modules/mcp/chat-extension/serverLabel'

interface ToolCallPendingApprovalContentProps {
  toolCall: McpToolCall
}

/**
 * Deterministic id of the built-in App Control MCP server
 * (`Uuid::new_v5(NAMESPACE_URL, "control.ziee.internal")`). Stable across
 * deployments; mirrors the backend `control_mcp_server_id()`.
 */
const CONTROL_MCP_SERVER_ID = 'd878787e-aa48-5f16-a31f-673052083f34'

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

  // The built-in App Control server (`invoke_capability`) ALWAYS re-prompts on a
  // state-changing action — the backend deliberately ignores any persisted
  // per-conversation auto-approval for it (mutations to the app are always
  // confirmed). So the "Approve for this conversation" affordance would be a
  // silent no-op here; hide it to avoid misleading the user. Gate on the control
  // server's deterministic id (Uuid v5 of "control.ziee.internal") — NOT the tool
  // name alone, which a third-party server could also expose.
  const isControlWrite =
    toolCall.server_id === CONTROL_MCP_SERVER_ID &&
    toolCall.tool_name === 'invoke_capability'

  const handleApproveOnce = async () => {
    setIsSubmitting(true)
    const mcpStore = Stores.McpComposer
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
      // A failed POST sets Chat.store.error synchronously before sendMessage
      // resolves, so poll once to revert the optimistic update on that failure.
      // (A generation error after a successful POST arrives later on the chat
      // stream and surfaces in the conversation, not by reverting this panel.)
      const chatError = Stores.Chat.$.error
      if (chatError) {
        throw new Error(chatError)
      }
      console.log('[MCP Approval] Tool approved once:', toolCall.tool_name)
    } catch (error) {
      console.error('[MCP Approval] Failed to approve tool:', error)
      // Revert optimistic update so the approval panel reappears on failure
      mcpStore.updateToolCall(toolCall.tool_use_id, {
        status: 'pending_approval',
      })
      setIsSubmitting(false)
    }
  }

  const handleApproveForConversation = async () => {
    setIsSubmitting(true)
    const mcpStore = Stores.McpComposer
    // Optimistic status update (same rationale as handleApproveOnce)
    mcpStore.updateToolCall(toolCall.tool_use_id, { status: 'started' })
    try {
      const chatState = Stores.Chat.$
      const conversationId = chatState.conversation?.id || null

      // 1. Add tool to auto_approved_tools for this conversation
      if (toolCall.server_id) {
        mcpStore.toggleAutoApprovedTool(
          conversationId,
          toolCall.server_id,
          toolCall.tool_name,
        )

        // 2. Persist to backend if conversation exists
        if (conversationId) {
          const mcpServerState = Stores.McpServer.$
          const availableServerIds = (mcpServerState?.servers || [])
            .filter((s: { enabled: boolean }) => s.enabled)
            .map((s: { id: string }) => s.id)
          await mcpStore.saveConversationConfig(
            conversationId,
            availableServerIds,
            undefined,
            true,
          )
        }
      }

      // 3. Approve current tool call
      mcpStore.addApprovalDecision({
        tool_use_id: toolCall.tool_use_id,
        decision: 'approve',
        note: 'User approved tool for this conversation',
      })

      await Stores.Chat.sendMessage()
      const chatError = Stores.Chat.$.error
      if (chatError) {
        throw new Error(chatError)
      }
      console.log(
        '[MCP Approval] Tool approved for conversation:',
        toolCall.tool_name,
      )
    } catch (error) {
      console.error('[MCP Approval] Failed to approve tool:', error)
      mcpStore.updateToolCall(toolCall.tool_use_id, {
        status: 'pending_approval',
      })
      setIsSubmitting(false)
    }
  }

  const handleDeny = async () => {
    setIsSubmitting(true)
    const mcpStore = Stores.McpComposer
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
      mcpStore.updateToolCall(toolCall.tool_use_id, {
        status: 'pending_approval',
      })
      setIsSubmitting(false)
    }
  }

  return (
    <div className="my-2" data-testid={`tool-approval-${toolCall.tool_use_id}`}>
      <Card
        size="sm"
        className="mb-2"
        data-testid="mcp-tool-approval-card"
        footer={
          // Right-aligned, negative-left like the elicitation form's Decline/Submit.
          <div className="flex w-full justify-end gap-2">
            <Button
              variant="outline"
              icon={<X />}
              onClick={handleDeny}
              loading={isSubmitting}
              size="default"
              data-testid="tool-approval-deny"
            >
              Deny
            </Button>
            <Button
              icon={<Check />}
              onClick={handleApproveOnce}
              loading={isSubmitting}
              size="default"
              data-testid="tool-approval-approve-once"
            >
              Approve once
            </Button>
            {!isControlWrite && (
              <Button
                icon={<Check />}
                onClick={handleApproveForConversation}
                loading={isSubmitting}
                size="default"
                data-testid="tool-approval-approve-conv"
              >
                Approve for this conversation
              </Button>
            )}
          </div>
        }
      >
        {/* Header row — status icon + tool name + server label, mirroring the
            elicitation Card's header. */}
        <div className="flex items-center gap-2 min-w-0">
          <Clock className="size-4 shrink-0 text-warning" />
          <Text strong className="truncate">{toolCall.tool_name}</Text>
          {mcpServerParenLabel(toolCall.server) && (
            <Text type="secondary" className="text-xs whitespace-nowrap">
              {mcpServerParenLabel(toolCall.server)}
            </Text>
          )}
          <Text type="secondary" className="text-xs whitespace-nowrap">
            — needs approval
          </Text>
        </div>

        <div className="mt-2">
          <Text className="text-sm">
            This tool requires your approval before execution.
          </Text>

          {toolCall.input !== undefined && (
            <div className="mt-2">
              <Text strong className="text-xs">
                Arguments:
              </Text>
              <pre className="p-2 rounded mt-1 overflow-auto max-h-40 text-xs bg-muted">
                {JSON.stringify(toolCall.input, null, 2)}
              </pre>
            </div>
          )}
        </div>
      </Card>
    </div>
  )
}
