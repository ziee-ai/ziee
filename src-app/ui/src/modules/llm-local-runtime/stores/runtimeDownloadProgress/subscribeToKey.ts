import type { StoreSet } from '@ziee/framework/store-kit'
import { ApiClient } from '@/api-client'
import type { RuntimeDownloadProgressState } from './state'
import type { RuntimeEngine } from '@/modules/llm-local-runtime/types'
import { RuntimeUpdate as RuntimeUpdateStore } from '@/modules/llm-local-runtime/stores/runtimeUpdate'
import { RuntimeVersion as RuntimeVersionStore } from '@/modules/llm-local-runtime/stores/runtimeVersion'

/**
 * Per-key abort controllers so we can tear down stale SSE subscriptions.
 * Module-scope (not in state) because they're not serializable / reactive.
 */
const sseAborts = new Map<string, AbortController>()

function percentOf(received: number, total: number | undefined): number | undefined {
  if (!total || total === 0) return undefined
  return Math.min(100, Math.max(0, (received / total) * 100))
}

/**
 * Factory: returns a `subscribeToKey` bound to the provided store methods.
 *
 * The SSE callbacks fire outside action closures, so they need the raw
 * `setState` to update the store (same as the original code used
 * `useRuntimeDownloadProgressStore.setState`).  `dismissKey` is passed
 * through because `subscribeToKey` must not import the store module
 * directly (circular: index → defineStore → actions → subscribeToKey →
 * store actions).
 */
export default function subscribeToKeyFactory(
  setState: StoreSet<RuntimeDownloadProgressState>,
  dismissKey: (key: string) => void,
) {
  return function subscribeToKey(key: string): void {
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
            setState(state => {
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
            setState(state => {
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
            RuntimeVersionStore.loadVersions().catch(() => {})
            const engine = key.split('@')[0] as RuntimeEngine
            RuntimeUpdateStore.checkForUpdates(engine).catch(() => {})
            // Auto-dismiss after a short delay so the card fades out.
            window.setTimeout(() => {
              dismissKey(key)
            }, 2000)
            sseAborts.delete(key)
          },
          failed: (data: { error: string }) => {
            setState(state => {
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
}
