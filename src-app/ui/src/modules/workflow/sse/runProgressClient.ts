import { ApiClient } from '@/api-client'
import type {
  SSEConnectedData,
  SSEElicitationRequiredData,
  SSEElicitationResolvedData,
  SSERunCancelledData,
  SSERunCompletedData,
  SSERunFailedData,
  SSERunStartedData,
  SSESnapshotData,
  SSEStepCompletedData,
  SSEStepFailedData,
  SSEStepItemProgressData,
  SSEStepStartedData,
} from '@/api-client/types'

/**
 * Callbacks the run-progress store wires into a per-run SSE stream.
 * Every field is optional so consumers subscribe to only what they
 * render.
 */
export interface RunProgressHandlers {
  connected?: (d: SSEConnectedData) => void
  snapshot?: (d: SSESnapshotData) => void
  runStarted?: (d: SSERunStartedData) => void
  stepStarted?: (d: SSEStepStartedData) => void
  stepItemProgress?: (d: SSEStepItemProgressData) => void
  stepCompleted?: (d: SSEStepCompletedData) => void
  stepFailed?: (d: SSEStepFailedData) => void
  elicitationRequired?: (d: SSEElicitationRequiredData) => void
  elicitationResolved?: (d: SSEElicitationResolvedData) => void
  runCompleted?: (d: SSERunCompletedData) => void
  runCancelled?: (d: SSERunCancelledData) => void
  runFailed?: (d: SSERunFailedData) => void
  /** Fired when the stream errors / disconnects (before reconnect). */
  disconnected?: () => void
}

export interface RunProgressSubscription {
  /** Tear down the stream + cancel any pending reconnect. */
  close: () => void
}

const MAX_RECONNECT_ATTEMPTS = 5
const RECONNECT_DELAY_MS = 3000

const TERMINAL_STATUSES = new Set(['completed', 'failed', 'cancelled'])

/**
 * Opens `GET /api/workflow-runs/{runId}/events` and routes the
 * WorkflowRunEvent stream to the supplied handlers. On disconnect it
 * reconnects with a fresh snapshot (the server re-sends a `snapshot`
 * frame on every (re)connect, so the view re-syncs without losing
 * state). Reconnect stops once a terminal RunCompleted/Failed/Cancelled
 * arrives, on `close()`, or after MAX_RECONNECT_ATTEMPTS.
 */
export function subscribeRunProgress(
  runId: string,
  handlers: RunProgressHandlers,
): RunProgressSubscription {
  let controller: AbortController | null = null
  let reconnectTimer: ReturnType<typeof setTimeout> | null = null
  let attempts = 0
  let terminal = false
  let closed = false

  const markTerminal = () => {
    terminal = true
  }

  const open = () => {
    if (closed || terminal) return
    void ApiClient.Workflow.subscribeRunEvents({ run_id: runId }, {
      SSE: {
        __init: ({ abortController }: { abortController: AbortController }) => {
          controller = abortController
          attempts = 0
        },
        connected: (d: SSEConnectedData) => handlers.connected?.(d),
        snapshot: (d: SSESnapshotData) => {
          handlers.snapshot?.(d)
          if (TERMINAL_STATUSES.has(d.status)) markTerminal()
        },
        runStarted: (d: SSERunStartedData) => handlers.runStarted?.(d),
        stepStarted: (d: SSEStepStartedData) => handlers.stepStarted?.(d),
        stepItemProgress: (d: SSEStepItemProgressData) =>
          handlers.stepItemProgress?.(d),
        stepCompleted: (d: SSEStepCompletedData) => handlers.stepCompleted?.(d),
        stepFailed: (d: SSEStepFailedData) => handlers.stepFailed?.(d),
        elicitationRequired: (d: SSEElicitationRequiredData) =>
          handlers.elicitationRequired?.(d),
        elicitationResolved: (d: SSEElicitationResolvedData) =>
          handlers.elicitationResolved?.(d),
        runCompleted: (d: SSERunCompletedData) => {
          handlers.runCompleted?.(d)
          markTerminal()
        },
        runCancelled: (d: SSERunCancelledData) => {
          handlers.runCancelled?.(d)
          markTerminal()
        },
        runFailed: (d: SSERunFailedData) => {
          handlers.runFailed?.(d)
          markTerminal()
        },
        default: () => {
          // Ignore unknown event types — forward-compatible.
        },
      },
    } as any).catch(() => {
      // Stream errored / server bounced. Reconnect with backoff unless
      // we've hit a terminal state or were explicitly closed.
      controller = null
      handlers.disconnected?.()
      if (closed || terminal) return
      attempts += 1
      if (attempts > MAX_RECONNECT_ATTEMPTS) return
      reconnectTimer = setTimeout(open, RECONNECT_DELAY_MS)
    })
  }

  open()

  return {
    close: () => {
      closed = true
      controller?.abort()
      controller = null
      if (reconnectTimer) {
        clearTimeout(reconnectTimer)
        reconnectTimer = null
      }
    },
  }
}
