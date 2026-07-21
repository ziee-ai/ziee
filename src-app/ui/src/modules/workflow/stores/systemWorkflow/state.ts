import type { StoreSet } from '@ziee/framework/store-kit'
import type { Workflow } from '@/api-client/types'

export const systemWorkflowState = {
  systemWorkflows: [] as Workflow[],
  isInitialized: false,
  loading: false,
  creating: false,
  error: null as string | null,
  // Per-workflow assigned group ids (lazy-loaded by the assignment card).
  groups: {} as Record<string, { groupIds: string[]; loading: boolean }>,
}

export type SystemWorkflowState = typeof systemWorkflowState
export type SystemWorkflowSet = StoreSet<SystemWorkflowState>
export type SystemWorkflowGet = () => SystemWorkflowState
