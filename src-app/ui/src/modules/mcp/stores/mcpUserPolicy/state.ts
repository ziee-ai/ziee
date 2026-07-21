import type { McpUserPolicy as McpUserPolicyRow } from '@/api-client/types'
import type { StoreSet } from '@ziee/framework/store-kit'

export const mcpUserPolicyState = {
  policy: null as McpUserPolicyRow | null,
  loading: false,
  error: null as string | null,
  isInitialized: false,
}

export type McpUserPolicyState = typeof mcpUserPolicyState
export type McpUserPolicySet = StoreSet<McpUserPolicyState>
export type McpUserPolicyGet = () => McpUserPolicyState
