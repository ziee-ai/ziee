import type { Workflow } from '@/api-client/types'
import type { WorkflowDrawerSet, WorkflowDrawerGet } from '../state'

export default (set: WorkflowDrawerSet, _get: WorkflowDrawerGet) =>
  (workflow: Workflow) => {
    set(d => {
      d.isOpen = true
      d.workflow = workflow
    })
  }
