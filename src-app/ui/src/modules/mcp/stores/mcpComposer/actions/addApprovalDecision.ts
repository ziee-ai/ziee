import { addApprovalDecisionTo } from '../../approvalRouting'
import type { McpComposerSet, McpComposerGet } from '../state'
import type { ToolApprovalDecision } from '@/api-client/types'

/** Add an approval decision (will be sent with next message). */
export default (set: McpComposerSet, _get: McpComposerGet) => (
  conversationKey: string,
  decision: ToolApprovalDecision,
) => {
  set(state => {
    state.approvalDecisions = addApprovalDecisionTo(
      state.approvalDecisions,
      conversationKey,
      decision,
    )
  })
  console.log(
    '[MCP Store] Added approval decision:',
    conversationKey,
    decision.decision,
    decision.tool_use_id,
  )
}
