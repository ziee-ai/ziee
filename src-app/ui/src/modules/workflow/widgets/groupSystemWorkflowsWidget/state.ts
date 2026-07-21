import type { Workflow } from '@/api-client/types'
import type { StoreSet } from '@ziee/framework/store-kit'

export const groupSystemWorkflowsWidgetState = {
  groupWorkflows: new Map<string, GroupWorkflows>(),
}

interface GroupWorkflows {
  groupId: string
  workflows: Workflow[]
  loading: boolean
  error: string | null
  lastFetched: number | null
}

export type GroupSystemWorkflowsWidgetState = typeof groupSystemWorkflowsWidgetState
export type GroupSystemWorkflowsWidgetSet = StoreSet<GroupSystemWorkflowsWidgetState>
export type GroupSystemWorkflowsWidgetGet = () => GroupSystemWorkflowsWidgetState
