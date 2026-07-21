import type { StoreSet } from '@ziee/framework/store-kit'
import type {
  DryRunResult,
  TestRunResponse,
  ValidateWorkflowResponse,
  Workflow,
  WorkflowRunStartResponse,
} from '@/api-client/types'

export const workflowState = {
  workflows: [] as Workflow[],
  isInitialized: false,
  loading: false,
  creating: false,
  error: null as string | null,
  operationsLoading: {} as Record<string, boolean>,
}

export type WorkflowState = typeof workflowState
export type WorkflowSet = StoreSet<WorkflowState>
export type WorkflowGet = () => WorkflowState

// Re-export API types so action files can import from '../state' instead of
// duplicating the api-client import.
export type {
  DryRunResult,
  TestRunResponse,
  ValidateWorkflowResponse,
  Workflow,
  WorkflowRunStartResponse,
}
