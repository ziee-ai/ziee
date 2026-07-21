import { ApiClient } from '@/api-client'
import { claimSubscription, percentOf } from '../../downloadProgress.helpers'
import type { VoiceModelDownloadProgressSet } from '../state'
import { VoiceModelUpdate } from '@/modules/voice/stores/voiceModelUpdate'
import { VoiceModel } from '@/modules/voice/stores/voiceModel'

/** Per-key abort controllers so we can tear down stale SSE subscriptions. */
const sseAborts = new Map<string, AbortController>()

interface SubscribeToKeyHandleFn {
  (key: string): void
  abort: (key: string) => void
}

export default (
  set: VoiceModelDownloadProgressSet,
  dismissEntry: (key: string) => void,
): SubscribeToKeyHandleFn => {
  // Claim the key SYNCHRONOUSLY so a rapid second call is deduped — the real
  // AbortController arrives later in the async `__init` callback. See the
  // sibling VoiceDownloadProgress for the race rationale.
  const fn: SubscribeToKeyHandleFn = ((key: string) => {
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
            set(state => {
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
            set(state => {
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
            VoiceModel.loadInstalled().catch(() => {
              /* non-fatal */
            })
            VoiceModelUpdate.checkForUpdates().catch(() => {
              /* non-fatal */
            })
            window.setTimeout(() => {
              dismissEntry(key)
            }, 2000)
            sseAborts.delete(key)
          },
          failed: (data: { error: string }) => {
            set(state => {
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
  }) as SubscribeToKeyHandleFn

  fn.abort = (key: string) => {
    sseAborts.get(key)?.abort()
    sseAborts.delete(key)
  }

  return fn
}
