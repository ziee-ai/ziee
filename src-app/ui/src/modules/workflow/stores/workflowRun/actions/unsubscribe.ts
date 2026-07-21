import type { WorkflowRunGet, WorkflowRunSet } from '../state'
import { subscriptions } from '../state'

// Action factory — unsubscribe doesn't mutate state, only closes the SSE socket.
export default (_set: WorkflowRunSet, _get: WorkflowRunGet) => {
  return (runId: string) => {
    subscriptions[runId]?.close()
    delete subscriptions[runId]
  }
}
