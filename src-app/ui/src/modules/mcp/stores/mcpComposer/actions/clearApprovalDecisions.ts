import { clearApprovalDecisionsIn } from '../../approvalRouting'
import type { McpComposerSet, McpComposerGet } from '../state'

/** Clear ONE conversation's approval decisions (after that conversation sends). */
export default (set: McpComposerSet, _get: McpComposerGet) => (conversationKey: string) => {
  set(state => {
    state.approvalDecisions = clearApprovalDecisionsIn(
      state.approvalDecisions,
      conversationKey,
    )
  })
  console.log('[MCP Store] Cleared approval decisions for', conversationKey)
}
