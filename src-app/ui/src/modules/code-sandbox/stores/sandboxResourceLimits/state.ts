import type { StoreSet } from '@ziee/framework/store-kit'
import type { CodeSandboxResourceLimits } from '@/api-client/types'

export const sandboxResourceLimitsState = {
  limits: null as CodeSandboxResourceLimits | null,
  loading: false,
  saving: false,
  error: null as string | null,
}

export type SandboxResourceLimitsState = typeof sandboxResourceLimitsState
export type SandboxResourceLimitsSet = StoreSet<SandboxResourceLimitsState>
export type SandboxResourceLimitsGet = () => SandboxResourceLimitsState
