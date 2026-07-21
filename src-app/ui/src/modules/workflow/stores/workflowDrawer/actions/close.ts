import type { WorkflowDrawerSet, WorkflowDrawerGet } from '../state'

export default (set: WorkflowDrawerSet, _get: WorkflowDrawerGet) => () => {
  set(d => {
    d.isOpen = false
  })
}
