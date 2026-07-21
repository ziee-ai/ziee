import type { McpComposerGet } from '../state'

/**
 * Get branch pending approvals from backend.
 */
export default (_set: unknown, _get: McpComposerGet) => async (branchId: string) => {
  const { ApiClient } = await import('@/api-client')
  return await ApiClient.Branch.getPendingApprovals({
    branch_id: branchId,
  })
}
