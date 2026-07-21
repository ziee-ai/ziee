import { ApiClient } from '@/api-client'
import type { ActionState, SandboxRootfsVersionsSet } from '../state'
import { sseState } from '../_sse'

function taskRowKey(t: { version: string; arch: string; flavor: string; package: string }): string {
  return `${t.version}::${t.arch}::${t.flavor}::${t.package}`
}

function setAction(s: { actions: Record<string, ActionState> }, key: string, patch: ActionState) {
  const cur = s.actions[key] ?? {}
  s.actions[key] = { ...cur, ...patch }
}

function clearAction(s: { actions: Record<string, ActionState> }, key: string) {
  delete s.actions[key]
}

// Wires from init: called by SSE handlers and reconnect timer.
let _wires: {
  loadStatus: () => Promise<void>
  self: () => Promise<void>
} | null = null

export function setSubscribeWires(wires: typeof _wires) {
  _wires = wires
}

export default (set: SandboxRootfsVersionsSet) => {
  return async () => {
    const wires = _wires!
    const st = sseState
    // Audit Net3: hard guard against double-subscribe (open stream OR pending
    // reconnect timer).
    if (st.controller || st.reconnectTimer) return
    try {
      await ApiClient.CodeSandbox.subscribeRootfsInstallProgress(undefined, {
        SSE: {
          __init: ({ abortController }: { abortController: AbortController }) => {
            st.controller = abortController
            st.reconnectAttempts = 0
          },
          connected: (_d: unknown) => {
            set(s => {
              s.sseConnected = true
              s.sseError = null
            })
          },
          taskStarted: (t: unknown) => {
            const key = taskRowKey(t as { version: string; arch: string; flavor: string; package: string })
            set(s => {
              s.installTasks[key] = t as never
              setAction(s, key, { installing: true })
            })
          },
          taskState: (t: unknown) => {
            const key = taskRowKey(t as { version: string; arch: string; flavor: string; package: string })
            const task = t as { status?: string }
            set(s => {
              s.installTasks[key] = t as never
              if (task.status === 'running') {
                setAction(s, key, { installing: true })
              } else if (task.status === 'completed' || task.status === 'failed') {
                // A reconnect snapshot can deliver an already-terminal task
                // whose event fired while disconnected — clear the installing
                // flag or the row stays stuck spinning.
                clearAction(s, key)
              }
            })
            if (task.status === 'completed') void wires.loadStatus()
          },
          progress: (d: unknown) => {
            set(s => {
              for (const key of Object.keys(s.installTasks)) {
                const t = s.installTasks[key] as { task_id?: string; status?: string; phase?: string; message?: string }
                if (t.task_id === (d as { task_id: string }).task_id) {
                  // Don't let a late progress event resurrect a terminal task.
                  if (t.status === 'completed' || t.status === 'failed') continue
                  t.phase = (d as { phase: string }).phase
                  t.message = (d as { message: string }).message
                }
              }
            })
          },
          complete: (d: unknown) => {
            set(s => {
              for (const key of Object.keys(s.installTasks)) {
                const t = s.installTasks[key] as { task_id?: string; status?: string; phase?: string; artifact_id?: string; bytes_downloaded?: number; duration_ms?: number }
                if (t.task_id === (d as { task_id: string }).task_id) {
                  // Keep it 'complete' (→100%) so the aggregate bar stays
                  // monotonic across the loadStatus round-trip; loadStatus
                  // prunes once the artifact lands.
                  t.status = 'completed'
                  t.phase = 'complete'
                  t.artifact_id = (d as { artifact_id: string }).artifact_id
                  t.bytes_downloaded = (d as { bytes_downloaded: number }).bytes_downloaded
                  t.duration_ms = (d as { duration_ms: number }).duration_ms
                  clearAction(s, key)
                }
              }
            })
            void wires.loadStatus()
          },
          failed: (d: unknown) => {
            set(s => {
              for (const key of Object.keys(s.installTasks)) {
                const t = s.installTasks[key] as { task_id?: string; status?: string; phase?: string; error?: string }
                if (t.task_id === (d as { task_id: string }).task_id) {
                  t.status = 'failed'
                  t.phase = 'failed'
                  t.error = (d as { error: string }).error
                  clearAction(s, key)
                }
              }
              // Surface via the per-version progress message only (setting the
              // global s.error too produced a duplicate top-level Alert).
            })
          },
          error: (msg: string) => {
            set(s => {
              s.sseConnected = false
              s.sseError = msg
            })
          },
          default: (_event: string, _data: unknown) => {},
        },
      } as any)
    } catch (e) {
      // Audit Net3: bounded reconnect (mirrors LlmModelDownload).
      st.controller = null
      set(s => {
        s.sseConnected = false
      })
      st.reconnectAttempts += 1
      if (st.reconnectAttempts < st.maxReconnectAttempts) {
        set(s => {
          s.sseError = `SSE disconnected; reconnecting (attempt ${st.reconnectAttempts}/${st.maxReconnectAttempts})`
        })
        st.reconnectTimer = setTimeout(() => {
          st.reconnectTimer = null
          void wires.self()
        }, st.reconnectDelayMs)
      } else {
        set(s => {
          s.sseError =
            e instanceof Error
              ? `SSE failed after ${st.maxReconnectAttempts} attempts: ${e.message}`
              : `SSE failed after ${st.maxReconnectAttempts} attempts`
        })
        st.reconnectAttempts = 0
      }
    }
  }
}
