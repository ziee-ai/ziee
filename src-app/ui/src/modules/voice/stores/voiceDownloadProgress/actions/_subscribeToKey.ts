import { ApiClient } from '@/api-client'
import type { VoiceDownloadProgressGet, VoiceDownloadProgressSet } from '../state'
import { claimSubscription, percentOf } from '../../downloadProgress.helpers'
import { VoiceRuntimeVersion } from '@/modules/voice/stores/voiceRuntimeVersion'
import { VoiceUpdate } from '@/modules/voice/stores/voiceUpdate'

/** SSE abort-controller map (module-level, page-reload-safe). */
const sseAborts = new Map<string, AbortController>()

type DismissEntryFn = (key: string) => Promise<void>

export default (
  set: VoiceDownloadProgressSet,
  _get: VoiceDownloadProgressGet,
  dismissEntry: DismissEntryFn,
) => {
  return (key: string): void => {
    // Claim the key SYNCHRONOUSLY so a rapid second call is deduped — the real
    // AbortController arrives later in the async `__init` callback, and without a
    // synchronous placeholder a `has(key)` guard is racy (two calls both pass it
    // before either sets the entry). `claimSubscription` returns false when
    // already claimed → no double-subscribe.
    if (!claimSubscription(sseAborts, key)) return
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
            set(state => {
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
            set(state => {
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
            VoiceRuntimeVersion.loadVersions().catch(() => {})
            VoiceUpdate.checkForUpdates().catch(() => {})
            window.setTimeout(() => {
              void dismissEntry(key)
            }, 2000)
            sseAborts.delete(key)
          },
          failed: (data: { error: string }) => {
            set(state => {
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
}
