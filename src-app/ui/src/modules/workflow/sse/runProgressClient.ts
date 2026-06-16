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

// Backoff caps. The per-attempt delay grows linearly with the
// consecutive-failure count (BASE * attempt), clamped to MAX_DELAY_MS,
// so a flapping server is probed with increasing space between tries.
const MAX_RECONNECT_ATTEMPTS = 5
const BASE_RECONNECT_DELAY_MS = 3000
const MAX_RECONNECT_DELAY_MS = 30000
// Hard ceiling on total reconnects across the whole subscription so an
// accept-then-drop server (which would otherwise reset `attempts` on
// every successful connect) can't loop forever.
const MAX_TOTAL_RECONNECTS = 20

const TERMINAL_STATUSES = new Set(['completed', 'failed', 'cancelled'])

/**
 * Opens `GET /api/workflow-runs/{runId}/events` and routes the
 * WorkflowRunEvent stream to the supplied handlers. On disconnect it
 * reconnects with backoff and a fresh snapshot (the server re-sends a
 * `snapshot` frame on every (re)connect, so the view re-syncs without
 * losing state). Reconnect stops once a terminal
 * RunCompleted/Failed/Cancelled arrives, on `close()`, after
 * MAX_RECONNECT_ATTEMPTS consecutive failures, or after
 * MAX_TOTAL_RECONNECTS total reconnects.
 *
 * `attempts` (the consecutive-failure counter that drives backoff +
 * the per-burst cap) is reset only once a frame is actually RECEIVED
 * (first `connected`/`snapshot` event) — NOT on raw connect — so a
 * server that accepts the connection then immediately drops it can't
 * reset the counter and loop forever.
 */
export function subscribeRunProgress(
  runId: string,
  handlers: RunProgressHandlers,
): RunProgressSubscription {
  let controller: AbortController | null = null
  let reconnectTimer: ReturnType<typeof setTimeout> | null = null
  let attempts = 0
  let totalReconnects = 0
  let terminal = false
  let closed = false

  const markTerminal = () => {
    terminal = true
  }

  // Called when a frame is genuinely received — proves the stream is
  // healthy, so the consecutive-failure backoff counter resets.
  const markFrameReceived = () => {
    attempts = 0
  }

  const open = () => {
    if (closed || terminal) return
    void ApiClient.Workflow.subscribeRunEvents({ run_id: runId }, {
      SSE: {
        __init: ({ abortController }: { abortController: AbortController }) => {
          controller = abortController
        },
        connected: (d: SSEConnectedData) => {
          markFrameReceived()
          handlers.connected?.(d)
        },
        snapshot: (d: SSESnapshotData) => {
          markFrameReceived()
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
      totalReconnects += 1
      if (
        attempts > MAX_RECONNECT_ATTEMPTS ||
        totalReconnects > MAX_TOTAL_RECONNECTS
      ) {
        return
      }
      const delay = Math.min(
        BASE_RECONNECT_DELAY_MS * attempts,
        MAX_RECONNECT_DELAY_MS,
      )
      reconnectTimer = setTimeout(open, delay)
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
