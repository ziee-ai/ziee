import { ApiClient } from '@/api-client'
import type { DownloadSnapshot } from '@/api-client/types'
import { defineStore } from '@/core/store-kit'
import { Stores } from '@/core/stores'
import type { RuntimeDownloadRequest, RuntimeEngine } from '../types'

/**
 * Per-(engine, version, backend) download progress, page-reload-safe.
 *
 * Backend downloads are detached on the server, so a page reload doesn't cancel
 * them. On mount, `loadActive()` pulls every in-flight + terminal task so the UI
 * repaints without waiting for the next SSE chunk; each non-terminal task opens
 * an SSE subscription. On Complete: refresh RuntimeVersion + RuntimeUpdate.
 */
export interface RuntimeDownloadProgressState {
  activeByKey: Map<string, DownloadSnapshot>
  loadingActive: boolean
  error: string | null
}

// Per-key abort controllers so we can tear down stale SSE subscriptions.
// Module-scope (not in state) because they're not serializable / reactive.
const sseAborts = new Map<string, AbortController>()

function percentOf(received: number, total: number | undefined): number | undefined {
  if (!total || total === 0) return undefined
  return Math.min(100, Math.max(0, (received / total) * 100))
}

export const RuntimeDownloadProgress = defineStore('RuntimeDownloadProgress', {
  state: {
    activeByKey: new Map<string, DownloadSnapshot>(),
    loadingActive: false,
    error: null as string | null,
  },
  actions: set => ({
    loadActive: async (): Promise<void> => {
      set({ loadingActive: true, error: null })
      try {
        const resp = await ApiClient.RuntimeVersion.listDownloads()
        const map = new Map<string, DownloadSnapshot>()
        for (const s of resp.downloads) map.set(s.key, s)
        set({ activeByKey: map, loadingActive: false })
        // Open SSE subscriptions for every non-terminal task so progress keeps
        // updating after a page reload.
        for (const s of resp.downloads) {
          if (s.status !== 'completed' && s.status !== 'failed') subscribeToKey(s.key)
        }
      } catch (e) {
        set({
          loadingActive: false,
          error: e instanceof Error ? e.message : 'Failed to load active downloads',
        })
      }
    },
    startDownload: async (req: RuntimeDownloadRequest): Promise<{ key: string }> => {
      const started = await ApiClient.RuntimeVersion.download(req)
      const key = started.key
      // Seed an in-progress entry immediately so the UI repaints before the
      // first SSE chunk lands.
      set(state => {
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
    // Removes a terminal entry once the UI has shown the final state.
    dismissEntry: (key: string): void =>
      set(state => {
        const next = new Map(state.activeByKey)
        next.delete(key)
        return { activeByKey: next }
      }),
    clearError: () => set({ error: null }),
  }),
  // `loadActive` runs on mount — the page-reload-survival hook.
  init: ({ actions }) => {
    void actions.loadActive()
  },
})

export const useRuntimeDownloadProgressStore = RuntimeDownloadProgress.store

/**
 * Open an SSE subscription for a download task key. Idempotent: a subsequent
 * call for the same key reuses the existing controller.
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
          // The backend's snapshot already populated us; no-op.
        },
        progress: (data: {
          status: string
          bytes_received: number
          total_bytes?: number
          percent?: number
        }) => {
          useRuntimeDownloadProgressStore.setState(state => {
            const next = new Map(state.activeByKey)
            const cur = next.get(key)
            if (!cur) return state
            next.set(key, {
              ...cur,
              status: data.status,
              bytes_received: data.bytes_received,
              total_bytes: data.total_bytes,
              percent: data.percent ?? percentOf(data.bytes_received, data.total_bytes),
            })
            return { activeByKey: next }
          })
        },
        complete: (data: { version_id: string; bytes_downloaded: number }) => {
          useRuntimeDownloadProgressStore.setState(state => {
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
          })
          // The new version row needs to land in RuntimeVersion + get re-diffed
          // by RuntimeUpdate.
          Stores.RuntimeVersion.loadVersions().catch(() => {})
          const engine = key.split('@')[0] as RuntimeEngine
          Stores.RuntimeUpdate.checkForUpdates(engine).catch(() => {})
          // Auto-dismiss after a short delay so the card fades out.
          window.setTimeout(() => {
            useRuntimeDownloadProgressStore.getState().dismissEntry(key)
          }, 2000)
          sseAborts.delete(key)
        },
        failed: (data: { error: string }) => {
          useRuntimeDownloadProgressStore.setState(state => {
            const next = new Map(state.activeByKey)
            const cur = next.get(key)
            if (cur) next.set(key, { ...cur, status: 'failed', error: data.error })
            return { activeByKey: next }
          })
          sseAborts.delete(key)
        },
      },
    },
  ).catch(() => {
    // Network or 404 (task evicted) — drop the controller so a future click can
    // re-subscribe.
    sseAborts.delete(key)
  })
}
