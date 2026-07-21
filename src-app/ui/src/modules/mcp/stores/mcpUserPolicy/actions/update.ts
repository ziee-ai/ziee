import { ApiClient } from '@/api-client'
import type { UpdateMcpUserPolicyRequest } from '@/api-client/types'
import type { McpUserPolicySet } from '../state'
import { emitMcpUserPolicyUpdated } from '@/modules/mcp/events/emitters'

export default (set: McpUserPolicySet) => async (req: UpdateMcpUserPolicyRequest) => {
  const policy = await ApiClient.McpUserPolicy.update(req)
  set(state => {
    state.policy = policy
    state.error = null
  })
  await emitMcpUserPolicyUpdated(
    policy.allowed_transports,
    policy.user_stdio_sandbox_flavor ?? null,
  )
}
