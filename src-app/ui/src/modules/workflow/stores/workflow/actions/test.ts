import { ApiClient } from '@/api-client'
import type { TestRunResponse, WorkflowGet, WorkflowSet } from '../state'

export default (_set: WorkflowSet, _get: WorkflowGet) => {
  return async (id: string, conversationId?: string): Promise<TestRunResponse> => {
    return await ApiClient.Workflow.test({
      id,
      ...(conversationId ? { conversation_id: conversationId } : {}),
    })
  }
}
