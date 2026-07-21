import type { SystemWorkflowGet, SystemWorkflowSet } from '../state'

export default (set: SystemWorkflowSet, _get: SystemWorkflowGet) =>
  async () => {
    set(draft => {
      draft.loading = false
      draft.error = null
    })
  }
