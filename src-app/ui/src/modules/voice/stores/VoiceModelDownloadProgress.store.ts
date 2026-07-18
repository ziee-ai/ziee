import { ApiClient } from '@/api-client'
import {
  type DownloadModelRequest,
  Permissions,
  type SnapshotDto,
} from '@/api-client/types'
import { hasPermissionNow } from '@/core/permissions'
import { defineStore } from '@ziee/framework/store-kit'
import { Stores } from '@ziee/framework/stores'
import { claimSubscription, percentOf } from './downloadProgress.helpers'

/**
 * Per-model whisper-model download progress, page-reload-safe. Mirrors the
 * sibling `VoiceDownloadProgress` (runtime binaries), keyed by the model
 * download `key`.
 *
 * Model downloads are detached, so a reload doesn't cancel them. On mount,
 * `loadActive()` pulls every in-flight + terminal task so the UI repaints
 * without waiting for the next SSE chunk; each non-terminal task opens an SSE
 * subscription. On Complete: refresh the installed-models library so the new
 * model appears in InstalledModelsCard.
 */

// Per-key abort controllers so we can tear down stale SSE subscriptions.
const sseAborts = new Map<string, AbortController>()

export const VoiceModelDownloadProgress = defineStore(
  'VoiceModelDownloadProgress',
  {
    state: {
      activeByKey: new Map<string, SnapshotDto>(),
      loadingActive: false,
      error: null as string | null,
    },
    actions: set => ({
      loadActive: async (): Promise<void> => {
        // Admin-only downloads endpoint; self-gate like the sibling voice stores
        // so non-admins don't 403 on app load.
        if (!hasPermissionNow(Permissions.VoiceAdminRead)) return
        set({ loadingActive: true, error: null })
        try {
          const downloads = await ApiClient.Voice.listModelDownloads()
          const map = new Map<string, SnapshotDto>()
          for (const s of downloads) map.set(s.key, s)
          set({ activeByKey: map, loadingActive: false })
          for (const s of downloads) {
            if (s.status !== 'completed' && s.status !== 'failed')
              subscribeToKey(s.key)
          }
        } catch (e) {
          set({
            loadingActive: false,
            error:
              e instanceof Error
                ? e.message
                : 'Failed to load active downloads',
          })
        }
      },
      startDownload: async (
        req: DownloadModelRequest,
      ): Promise<{ key: string }> => {
        const started = await ApiClient.Voice.downloadModel(req)
        const key = started.key
        set(state => {
          const next = new Map(state.activeByKey)
          next.set(key, {
            task_id: started.task_id,
            key,
            name: started.name,
            status: 'downloading',
            bytes_received: 0,
          })
          return { activeByKey: next }
        })
        subscribeToKey(key)
        return { key }
      },
      cancelDownload: async (key: string): Promise<void> => {
        try {
          await ApiClient.Voice.cancelModelDownload({ key })
        } finally {
          sseAborts.get(key)?.abort()
          sseAborts.delete(key)
        }
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
  },
)

export const useVoiceModelDownloadProgressStore =
  VoiceModelDownloadProgress.store

/** Open an SSE subscription for a download key. Idempotent per key. */
function subscribeToKey(key: string): void {
  // Claim the key SYNCHRONOUSLY so a rapid second call is deduped — the real
  // AbortController arrives later in the async `__init` callback. See the
  // sibling VoiceDownloadProgress for the race rationale.
  if (!claimSubscription(sseAborts, key)) return
  ApiClient.Voice.subscribeModelDownloadEvents(
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
          useVoiceModelDownloadProgressStore.setState(state => {
            const next = new Map(state.activeByKey)
            const cur = next.get(key)
            if (!cur) return state
            next.set(key, {
              ...cur,
              status: data.status,
              bytes_received: data.bytes_received,
              total_bytes: data.total_bytes,
              percent:
                data.percent ??
                percentOf(data.bytes_received, data.total_bytes),
            })
            return { activeByKey: next }
          })
        },
        complete: (data: { model_id: string; bytes_downloaded: number }) => {
          useVoiceModelDownloadProgressStore.setState(state => {
            const next = new Map(state.activeByKey)
            const cur = next.get(key)
            if (cur) {
              next.set(key, {
                ...cur,
                status: 'completed',
                bytes_received: data.bytes_downloaded || cur.bytes_received,
                percent: 100,
              })
            }
            return { activeByKey: next }
          })
          // Refresh the installed-models library + the catalog (installed flags).
          Stores.VoiceModel.loadInstalled().catch(() => {
            /* non-fatal */
          })
          Stores.VoiceModelUpdate.checkForUpdates().catch(() => {
            /* non-fatal */
          })
          window.setTimeout(() => {
            useVoiceModelDownloadProgressStore.getState().dismissEntry(key)
          }, 2000)
          sseAborts.delete(key)
        },
        failed: (data: { error: string }) => {
          useVoiceModelDownloadProgressStore.setState(state => {
            const next = new Map(state.activeByKey)
            const cur = next.get(key)
            if (cur)
              next.set(key, { ...cur, status: 'failed', error: data.error })
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
