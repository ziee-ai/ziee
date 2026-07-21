import { ApiClient } from '@/api-client'
import type { ValidateWorkflowResponse, WorkflowGet, WorkflowSet } from '../state'

export default (_set: WorkflowSet, _get: WorkflowGet) => {
  return async (yaml: string): Promise<ValidateWorkflowResponse> => {
    return await ApiClient.Workflow.validate({ workflow_yaml: yaml })
  }
}
