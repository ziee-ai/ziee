import { create } from 'zustand'
import { ApiClient } from '@/api-client'
import { Stores } from '@/core/stores'
import type { DownloadSnapshot } from '@/api-client/types'
import type { RuntimeDownloadRequest, RuntimeEngine } from '../types'

/**
 * Per-(engine, version, backend) download progress, page-reload-safe.
 *
 * Backend downloads are detached on the server (the runtime spawns a
 * `tokio::spawn` task in `download_task`), so a page reload doesn't
 * cancel the download — the in-process registry keeps pumping bytes.
 * This store:
 *
 *   1. On mount, `loadActive()` pulls every in-flight + terminal task
 *      from `GET /local-runtime/versions/downloads` so the UI repaints
 *      progress bars without waiting for the next SSE chunk.
 *   2. For each non-terminal task, opens an SSE subscription via
 *      `ApiClient.RuntimeVersion.subscribeDownloadEvents` and updates
 *      the per-key snapshot on every Progress event.
 *   3. `startDownload(req)` POSTs the detached-start endpoint and
 *      opens an SSE subscription for the returned key.
 *   4. On Complete: refreshes RuntimeVersion + RuntimeUpdate so the
 *      "Installed versions" list grows and the "Available versions"
 *      list re-flags the row as installed; keeps the completed
 *      snapshot around briefly so the bar can fade out gracefully.
 */
export interface RuntimeDownloadProgressState {
  activeByKey: Map<string, DownloadSnapshot>
  loadingActive: boolean
  error: string | null

  loadActive: () => Promise<void>
  startDownload: (req: RuntimeDownloadRequest) => Promise<{ key: string }>
  // Removes a terminal entry once the UI has had a chance to show
  // the final state (e.g. after a couple seconds).
  dismissEntry: (key: string) => void
  clearError: () => void

  __init__: {
    __store__: () => void
    activeByKey: () => Promise<void>
  }
}

// Per-key abort controllers so we can tear down stale SSE
// subscriptions on logout / store rebuild. Module-scope (not in the
// Zustand state) because they're not serializable + not reactive.
const sseAborts = new Map<string, AbortController>()

function percentOf(received: number, total: number | undefined): number | undefined {
  if (!total || total === 0) return undefined
  return Math.min(100, Math.max(0, (received / total) * 100))
}

export const useRuntimeDownloadProgressStore = create<RuntimeDownloadProgressState>(
  (set, get) => ({
    activeByKey: new Map(),
    loadingActive: false,
    error: null,

    loadActive: async (): Promise<void> => {
      set({ loadingActive: true, error: null })
      try {
        const resp = await ApiClient.RuntimeVersion.listDownloads()
        const map = new Map<string, DownloadSnapshot>()
        for (const s of resp.downloads) {
          map.set(s.key, s)
        }
        set({ activeByKey: map, loadingActive: false })
        // Open SSE subscriptions for every non-terminal task so
        // progress keeps updating after a page reload.
        for (const s of resp.downloads) {
          if (s.status !== 'completed' && s.status !== 'failed') {
            subscribeToKey(s.key)
          }
        }
      } catch (e) {
        set({
          loadingActive: false,
          error:
            e instanceof Error ? e.message : 'Failed to load active downloads',
        })
      }
    },

    startDownload: async (
      req: RuntimeDownloadRequest,
    ): Promise<{ key: string }> => {
      const started = await ApiClient.RuntimeVersion.download(req)
      const key = started.key
      // Seed an in-progress entry immediately so the UI repaints
      // the button + opens the progress bar before the first SSE
      // chunk lands.
      set((state: RuntimeDownloadProgressState) => {
        const next = new Map(state.activeByKey)
        next.set(key, {
          task_id: started.task_id,
          key,
          engine: started.engine,
          version: started.version,
          backend: started.backend,
          status: started.status,
          bytes_received: 0,
        })
        return { activeByKey: next }
      })
      subscribeToKey(key)
      return { key }
    },

    dismissEntry: (key: string): void =>
      set((state: RuntimeDownloadProgressState) => {
        const next = new Map(state.activeByKey)
        next.delete(key)
        return { activeByKey: next }
      }),

    clearError: () => set({ error: null }),

    __init__: {
      // `loadActive` runs as the init for `activeByKey` on module
      // mount — that's the page-reload-survival hook.
      __store__: () => {},
      activeByKey: () => get().loadActive(),
    },
  }),
)

/**
 * Open an SSE subscription for a download task key. Idempotent: a
 * subsequent call for the same key reuses the existing controller
 * (so a page-reload->loadActive flow doesn't double-subscribe when
 * the user is also actively clicking Download).
 */
function subscribeToKey(key: string): void {
  if (sseAborts.has(key)) return
  ApiClient.RuntimeVersion.subscribeDownloadEvents(
    { key },
    {
      SSE: {
        __init: ({ abortController }: { abortController: AbortController }) => {
          sseAborts.set(key, abortController)
        },
        connected: () => {
          // The backend's snapshot already populated us via
          // loadActive (or startDownload's seed); no-op.
        },
        progress: (data: {
          status: string
          bytes_received: number
          total_bytes?: number
          percent?: number
        }) => {
          useRuntimeDownloadProgressStore.setState(
            (state: RuntimeDownloadProgressState) => {
              const next = new Map(state.activeByKey)
              const cur = next.get(key)
              if (!cur) return state
              next.set(key, {
                ...cur,
                status: data.status,
                bytes_received: data.bytes_received,
                total_bytes: data.total_bytes,
                percent:
                  data.percent ?? percentOf(data.bytes_received, data.total_bytes),
              })
              return { activeByKey: next }
            },
          )
        },
        complete: (data: { version_id: string; bytes_downloaded: number }) => {
          useRuntimeDownloadProgressStore.setState(
            (state: RuntimeDownloadProgressState) => {
              const next = new Map(state.activeByKey)
              const cur = next.get(key)
              if (cur) {
                next.set(key, {
                  ...cur,
                  status: 'completed',
                  bytes_received: data.bytes_downloaded || cur.bytes_received,
                  percent: 100,
                  result_version_id: data.version_id,
                })
              }
              return { activeByKey: next }
            },
          )
          // The new version row needs to land in RuntimeVersion +
          // get re-diffed by RuntimeUpdate. Cheaper than emitting
          // a synthetic `runtime_version.created` event here.
          Stores.RuntimeVersion.loadVersions().catch(() => {})
          // Re-check the engine's upstream cache so the "installed"
          // tag flips on for this version in the available-versions
          // list. Derive the engine from the key prefix.
          const engine = key.split('@')[0] as RuntimeEngine
          Stores.RuntimeUpdate.checkForUpdates(engine).catch(() => {})
          // Auto-dismiss after a short delay so the progress card
          // fades out once the user has seen "100% — Completed".
          window.setTimeout(() => {
            useRuntimeDownloadProgressStore.getState().dismissEntry(key)
          }, 2000)
          sseAborts.delete(key)
        },
        failed: (data: { error: string }) => {
          useRuntimeDownloadProgressStore.setState(
            (state: RuntimeDownloadProgressState) => {
              const next = new Map(state.activeByKey)
              const cur = next.get(key)
              if (cur) {
                next.set(key, {
                  ...cur,
                  status: 'failed',
                  error: data.error,
                })
              }
              return { activeByKey: next }
            },
          )
          sseAborts.delete(key)
        },
      },
    },
  ).catch(() => {
    // Network or 404 (task evicted) — drop the controller so a
    // future click can re-subscribe.
    sseAborts.delete(key)
  })
}
