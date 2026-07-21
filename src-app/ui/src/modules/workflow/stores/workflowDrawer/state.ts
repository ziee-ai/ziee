import type { Workflow } from '@/api-client/types'
import type { StoreSet } from '@ziee/framework/store-kit'

export const workflowDrawerState = {
  isOpen: false,
  workflow: null as Workflow | null,
}

export type WorkflowDrawerState = typeof workflowDrawerState
export type WorkflowDrawerSet = StoreSet<WorkflowDrawerState>
export type WorkflowDrawerGet = () => WorkflowDrawerState
