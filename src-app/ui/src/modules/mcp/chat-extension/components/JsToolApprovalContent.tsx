import { useState } from 'react'
import { Alert, Button, Space, Text } from '@ziee/kit'
import { Check, Clock, X } from 'lucide-react'
import { Stores } from '@/core/stores'
import type { ContentRendererProps } from '@/modules/chat/core/extensions/types'
import { mcpServerParenLabel } from '@/modules/mcp/chat-extension/serverLabel'

/**
 * Renders the approve/deny prompt for a gated sub-tool call a `run_js` script
 * wants to make while it is SUSPENDED in-process. Unlike the turn-boundary MCP
 * approval flow, this resolves via the side-channel elicitation `/respond`
 * endpoint (the same in-process oneshot `ask_user` uses) so the live script
 * resumes — `accept` runs the sub-tool, `decline` throws `ToolApprovalDenied`
 * into the script. Injected as a `run_js_approval` content block by the
 * `runJsApprovalRequired` SSE handler.
 */
interface JsToolApprovalData {
  elicitation_id: string
  tool_name: string
  server: string
  input?: Record<string, unknown>
}

export function JsToolApprovalContent({ content }: ContentRendererProps) {
  const data = content.content as unknown as JsToolApprovalData
  const [submitting, setSubmitting] = useState(false)

  // Derive the resolved state from the elicitationRequests store (the live
  // source of truth), NOT local state: resolveElicitation flips the entry
  // optimistically and ROLLS IT BACK to 'pending' on a failed POST, so a failed
  // approve re-enables the buttons (no false "Approved") and the resolved state
  // survives a component remount (virtualized list / streaming→final swap).
  const status = Stores.McpComposer.elicitationRequests.get(data.elicitation_id)?.status
  const resolved: 'approved' | 'denied' | null =
    status === 'accepted' ? 'approved' : status === 'declined' || status === 'cancelled' ? 'denied' : null

  const resolve = async (action: 'accept' | 'decline') => {
    // Re-entrancy guard: never POST twice to a single-use elicitation.
    if (submitting || resolved !== null) return
    setSubmitting(true)
    try {
      // resolveElicitation reflects success/failure in the store entry; the
      // derived `resolved` above reacts (rollback → buttons return for retry).
      await Stores.McpComposer.resolveElicitation(data.elicitation_id, action)
    } finally {
      setSubmitting(false)
    }
  }

  const icon =
    resolved === 'approved' ? <Check /> : resolved === 'denied' ? <X /> : <Clock />

  return (
    <div className="my-2" data-testid={`run-js-approval-${data.elicitation_id}`}>
      <Alert
        tone={resolved === 'approved' ? 'success' : resolved === 'denied' ? 'neutral' : 'warning'}
        data-testid={`run-js-approval-alert-${data.elicitation_id}`}
        icon={icon}
        title={
          <div>
            <Text strong>run_js wants to call: {data.tool_name}</Text>
            {mcpServerParenLabel(data.server) && (
              <Text type="secondary" className="ms-2 text-xs whitespace-nowrap">
                {mcpServerParenLabel(data.server)}
              </Text>
            )}
          </div>
        }
        description={
          <div className="mt-2">
            <Text className="text-sm">
              A running script wants to call this tool. Approve to let the script continue.
            </Text>
            {data.input !== undefined && (
              <div className="mt-2">
                <Text strong className="text-xs">
                  Arguments:
                </Text>
                <pre className="p-2 rounded mt-1 overflow-auto max-h-40 text-xs bg-muted">
                  {JSON.stringify(data.input, null, 2)}
                </pre>
              </div>
            )}
            {resolved === null ? (
              <div className="mt-3">
                <Space>
                  <Button
                    icon={<Check />}
                    onClick={() => resolve('accept')}
                    loading={submitting}
                    size="default"
                    data-testid={`run-js-approval-approve-${data.elicitation_id}`}
                  >
                    Approve
                  </Button>
                  <Button
                    variant="destructive"
                    icon={<X />}
                    onClick={() => resolve('decline')}
                    loading={submitting}
                    size="default"
                    data-testid={`run-js-approval-deny-${data.elicitation_id}`}
                  >
                    Deny
                  </Button>
                </Space>
              </div>
            ) : (
              <Text
                type="secondary"
                className="mt-2 block text-xs"
                data-testid={`run-js-approval-status-${data.elicitation_id}`}
                data-status={resolved}
              >
                {resolved === 'approved' ? 'Approved — script resumed.' : 'Denied.'}
              </Text>
            )}
          </div>
        }
      />
    </div>
  )
}
