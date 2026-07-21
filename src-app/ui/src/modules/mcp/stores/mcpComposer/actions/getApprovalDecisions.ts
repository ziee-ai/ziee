import { getApprovalDecisionsFrom } from '../../approvalRouting'
import type { McpComposerGet } from '../state'
import type { ToolApprovalDecision } from '@/api-client/types'

/** Get the pending approval decisions for ONE conversation (ITEM-33) — synchronous. */
export default (_set: unknown, get: McpComposerGet): (conversationKey: string) => ToolApprovalDecision[] => {
  return (conversationKey: string) => {
    return getApprovalDecisionsFrom(get().approvalDecisions, conversationKey)
  }
}
