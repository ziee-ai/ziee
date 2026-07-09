import { ApiClient } from '@/api-client'
import type { DownloadSnapshot2, DownloadVersionRequest2 } from '@/api-client/types'
import { defineStore } from '@/core/store-kit'
import { Stores } from '@/core/stores'

/**
 * Per-(version, backend) whisper download progress, page-reload-safe. Mirrors
 * llm-local-runtime's `RuntimeDownloadProgress`.
 *
 * Backend downloads are detached, so a reload doesn't cancel them. On mount,
 * `loadActive()` pulls every in-flight + terminal task so the UI repaints
 * without waiting for the next SSE chunk; each non-terminal task opens an SSE
 * subscription. On Complete: refresh VoiceRuntimeVersion + VoiceUpdate.
 */

// Per-key abort controllers so we can tear down stale SSE subscriptions.
const sseAborts = new Map<string, AbortController>()

function percentOf(received: number, total: number | undefined): number | undefined {
  if (!total || total === 0) return undefined
  return Math.min(100, Math.max(0, (received / total) * 100))
}

export const VoiceDownloadProgress = defineStore('VoiceDownloadProgress', {
  state: {
    activeByKey: new Map<string, DownloadSnapshot2>(),
    loadingActive: false,
    error: null as string | null,
  },
  actions: set => ({
    loadActive: async (): Promise<void> => {
      set({ loadingActive: true, error: null })
      try {
        const resp = await ApiClient.Voice.listVersionDownloads()
        const map = new Map<string, DownloadSnapshot2>()
        for (const s of resp.downloads) map.set(s.key, s)
        set({ activeByKey: map, loadingActive: false })
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
    startDownload: async (req: DownloadVersionRequest2): Promise<{ key: string }> => {
      const started = await ApiClient.Voice.downloadVersion(req)
      const key = started.key
      set(state => {
        const next = new Map(state.activeByKey)
        next.set(key, {
          task_id: started.task_id,
          key,
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
      set(state => {
        const next = new Map(state.activeByKey)
        next.delete(key)
        return { activeByKey: next }
      }),
    clearError: () => set({ error: null }),
  }),
  init: ({ actions }) => {
    void actions.loadActive()
  },
})

export const useVoiceDownloadProgressStore = VoiceDownloadProgress.store

/** Open an SSE subscription for a download key. Idempotent per key. */
function subscribeToKey(key: string): void {
  if (sseAborts.has(key)) return
  ApiClient.Voice.subscribeVersionDownloadEvents(
    { key },
    {
      SSE: {
        __init: ({ abortController }: { abortController: AbortController }) => {
          sseAborts.set(key, abortController)
        },
        connected: () => {
          // The backend snapshot already populated us; no-op.
        },
        progress: (data: {
          status: string
          bytes_received: number
          total_bytes?: number
          percent?: number
        }) => {
          useVoiceDownloadProgressStore.setState(state => {
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
          useVoiceDownloadProgressStore.setState(state => {
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
          Stores.VoiceRuntimeVersion.loadVersions().catch(() => {})
          Stores.VoiceUpdate.checkForUpdates().catch(() => {})
          window.setTimeout(() => {
            useVoiceDownloadProgressStore.getState().dismissEntry(key)
          }, 2000)
          sseAborts.delete(key)
        },
        failed: (data: { error: string }) => {
          useVoiceDownloadProgressStore.setState(state => {
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
    sseAborts.delete(key)
  })
}
