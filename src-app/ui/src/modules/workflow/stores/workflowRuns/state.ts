import type { StoreSet } from '@ziee/framework/store-kit'
import type { WorkflowRunSummary } from '@/api-client/types'

export const workflowRunsState = {
  runs: {} as Record<string, WorkflowRunSummary[]>,
  loading: {} as Record<string, boolean>,
  deleting: {} as Record<string, boolean>,
}

export type WorkflowRunsState = typeof workflowRunsState
export type WorkflowRunsSet = StoreSet<WorkflowRunsState>
export type WorkflowRunsGet = () => WorkflowRunsState
