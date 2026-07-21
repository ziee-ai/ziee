import { ApiClient } from '@/api-client'
import type { DryRunResult, WorkflowGet, WorkflowSet } from '../state'

export default (_set: WorkflowSet, _get: WorkflowGet) => {
  return async (id: string, inputs: any): Promise<DryRunResult> => {
    return await ApiClient.Workflow.dryRun({ id, inputs })
  }
}
