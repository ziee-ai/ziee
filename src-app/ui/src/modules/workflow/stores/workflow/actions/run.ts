import { ApiClient } from '@/api-client'
import type { WorkflowGet, WorkflowSet } from '../state'
import type { WorkflowRunStartResponse } from '@/api-client/types'

export default (_set: WorkflowSet, _get: WorkflowGet) => {
  return async (
    id: string,
    inputs: any,
    conversationId?: string,
    mocks?: any,
    modelId?: string,
    captureLogs?: boolean,
  ): Promise<WorkflowRunStartResponse> => {
    return await ApiClient.Workflow.run({
      id,
      inputs,
      ...(conversationId ? { conversation_id: conversationId } : {}),
      ...(mocks ? { mocks } : {}),
      ...(modelId ? { model_id: modelId } : {}),
      ...(captureLogs ? { capture_logs: true } : {}),
    })
  }
}
