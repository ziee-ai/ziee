import { ApiClient } from '@/api-client'
import { useLlmProviderStore } from '@/modules/llm-provider/stores/llmProvider'
import { useLlmModelDownloadStore } from '@/modules/llm-provider/stores/llmModelDownload'
import type {
  DownloadInstance,
  DownloadProgressData,
  DownloadProgressUpdate,
  DownloadStatus,
  SSEDownloadProgressConnectedData,
} from '@/api-client/types'

import type { LlmModelDownloadGet, LlmModelDownloadSet } from '../state'
import loadExistingDownloadsFactory from './_loadExistingDownloads'
import {
  emitLlmModelDownloadCompleted,
  emitLlmModelDownloadFailed,
} from '@/modules/llm-provider/events/emitters'

/**
 * Merge one SSE progress frame into the stored download row.
 *
 * The wire event is **FLAT** — the server's `From<&DownloadInstance>` lifts
 * `current` / `total` / `speed_bps` / `eta_seconds` / `message` / `phase` to the
 * TOP level (`llm_model/handlers/downloads.rs`, pinned by TEST-9) — while every
 * surface that renders a download reads `download.progress_data.*`:
 * `DownloadItem` ("N Bytes / M Bytes"), `DownloadProgress` (the percent), and the
 * hub's `ModelHubCard`.
 *
 * This used to be `{ ...download, ...update } as DownloadInstance`, which grafted
 * the flat keys on as strays and left `progress_data` at whatever the initial
 * REST snapshot held — zeros for a just-started download. So the bar sat at 0%
 * and the byte counts read "0 Bytes / 0 Bytes" for an entire 5.68 GB transfer,
 * in the onboarding step AND the LLM-providers view, because both read this one
 * store. The `as DownloadInstance` cast is what stopped `tsc` reporting it; it is
 * gone, and this function is typed end-to-end instead.
 *
 * The progress FIGURES fall back to the value already on screen: the server
 * sends them as `Option`, and a `null` means "unknown right now", never "zero" —
 * blanking a figure the user is watching would be its own bug (TEST-9 pins that
 * the absent case really is `null` and not `0`). `error_message` and `model_id`
 * are the opposite: those carry the WHOLE ROW's value on every frame, so a null
 * there means genuinely cleared and is taken as-is. Getting that backwards left
 * stale red error text on a row whose error the server had cleared (audit FIX-5).
 */

/**
 * Exhaustive by construction: this is a `Record` keyed on the generated
 * `DownloadStatus` union, so adding a status server-side is a COMPILE error here
 * rather than a silent fall-through. A plain `readonly DownloadStatus[]` (what
 * this was) accepts a short list happily — the same "a literal that can drift
 * from its source" defect this file's own comments condemn for the CORS header
 * (audit FIX-6).
 */
const DOWNLOAD_STATUSES_EXHAUSTIVE: Record<DownloadStatus, true> = {
  pending: true,
  downloading: true,
  completed: true,
  failed: true,
  cancelled: true,
}
/**
 * Membership is tested against a `Set`, NOT with `in`. `wire in obj` walks the
 * prototype chain, so `'toString'`, `'constructor'`, `'valueOf'` and
 * `'__proto__'` all answer true and would be written onto the row as a bogus
 * status — a regression the first version of this rewrite introduced, and one
 * the "unrecognised status" test did not catch because its input was not a
 * prototype member (audit round 2).
 */
const DOWNLOAD_STATUSES = new Set<string>(Object.keys(DOWNLOAD_STATUSES_EXHAUSTIVE))

/**
 * The wire carries `status` as a bare `string` (the server stringifies its enum),
 * so it cannot be assigned to `DownloadInstance['status']` directly. Narrow it,
 * and keep the row's existing status for anything unrecognised rather than
 * writing a value the rest of the store would then compare against and miss —
 * the old `as DownloadInstance` cast is precisely what let this mismatch through.
 */
function narrowStatus(wire: string, previous: DownloadStatus): DownloadStatus {
  return DOWNLOAD_STATUSES.has(wire) ? (wire as DownloadStatus) : previous
}

export function applyProgressUpdate(
  download: DownloadInstance,
  update: DownloadProgressUpdate,
): DownloadInstance {
  const previous = download.progress_data
  // Everything the frame and the row jointly know about progress. All-absent
  // means the row has no progress yet.
  const phase = update.phase ?? previous?.phase
  const current = update.current ?? previous?.current
  const total = update.total ?? previous?.total
  const message = update.message ?? previous?.message
  const speed_bps = update.speed_bps ?? previous?.speed_bps
  const eta_seconds = update.eta_seconds ?? previous?.eta_seconds

  // Materialise `progress_data` only when something about progress is actually
  // known. `phase` is excluded because it is the one progress field the server
  // does NOT send as an `Option` — `From<&DownloadInstance>` fills it with
  // `Created` even for a row that has none — so including it made this
  // predicate unconditionally true.
  //
  // HONEST SCOPE (audit round 3): this guard is DEFENSIVE, not a fix for the
  // queued-download "0 Bytes / 0 Bytes" render. That render has a different
  // cause entirely: the row's INSERT (`llm_model/repository.rs`) seeds a
  // fully-zeroed `progress_data`, so both the REST snapshot and every SSE frame
  // carry `current: 0` rather than null, and a queued row therefore HAS progress
  // data as far as any consumer can tell. An earlier round claimed this guard
  // removed that symptom; it does not, and the claim is withdrawn rather than
  // restated. What the guard does do is keep a genuinely progress-less row
  // (`progress_data: null`, which the schema permits) from acquiring zeros here.
  const known =
    current !== undefined ||
    total !== undefined ||
    speed_bps !== undefined ||
    eta_seconds !== undefined ||
    message !== undefined

  const progress_data: DownloadProgressData | undefined = known
    ? {
        phase: phase ?? 'created',
        current: current ?? 0,
        total: total ?? 0,
        message: message ?? '',
        speed_bps: speed_bps ?? 0,
        eta_seconds: eta_seconds ?? 0,
      }
    : undefined

  return {
    ...download,
    status: narrowStatus(update.status, download.status),
    // Whole-row fields: a null is a CLEAR, not "unknown" (see the header).
    error_message: update.error_message,
    model_id: update.model_id,
    progress_data,
  }
}

export default (set: LlmModelDownloadSet, get: LlmModelDownloadGet) => {
  const loadExistingDownloads = loadExistingDownloadsFactory(set, get)

  const action: () => Promise<void> = async () => {
    const state = get()
    if (state.sseConnected) return

    try {
      await ApiClient.LlmModel.subscribeDownloadProgress(undefined, {
        SSE: {
          // Only the abort handle is knowable here: the transport dispatches
          // `__init` as soon as fetch() resolves and BEFORE it checks
          // response.ok, so a failing status reaches this callback too.
          // Marking the stream connected and resetting the retry counter here
          // made every failed attempt look like a fresh start — the catch
          // below would take it 0 → 1, forever short of maxAttempts — so the
          // bounded reconnect never terminated and re-hit the endpoint every
          // 3s indefinitely.
          __init: ({ abortController }) => {
            // Signal the abort controller so onCleanup can abort it.
            ;(globalThis as Record<string, unknown>).__LLM_DL_SSE_ABORT = abortController
          },
          connected: (_data: SSEDownloadProgressConnectedData) => {
            // The server's handshake, reachable only on a real 200 stream —
            // the one point at which the connection has genuinely succeeded.
            set({ sseConnected: true, sseError: null, reconnectAttempts: 0 })
          },
          update: (updates: DownloadProgressUpdate[]) => {
            const prevState = get()
            const prevStatusById = new Map<string, string>(
              prevState.downloads.map((d: DownloadInstance) => [d.id, d.status]),
            )
            const newlyCompleted = updates.filter((u) => u.status === 'completed')
            if (newlyCompleted.length > 0) {
              const providerIds = [
                ...new Set(
                  newlyCompleted
                    .map((d) => d.provider_id)
                    .filter((id): id is string => !!id),
                ),
              ]
              for (const providerId of providerIds) {
                void useLlmProviderStore.getState().loadModelsForProvider(providerId)
              }
            }
            for (const u of updates) {
              if (!u.id || typeof u.status !== 'string') continue
              const prev = prevStatusById.get(u.id)
              if (prev === u.status) continue
              const isNewlyTerminal =
                (u.status === 'completed' || u.status === 'failed') && prev !== undefined
              if (!isNewlyTerminal) continue
              const priorRow = prevState.downloads.find((d) => d.id === u.id)
              const displayName =
                priorRow?.request_data?.display_name ||
                priorRow?.request_data?.model_name ||
                'Model'
              if (u.status === 'completed') {
                void emitLlmModelDownloadCompleted(
                  u.id,
                  u.provider_id ?? priorRow?.provider_id ?? '',
                  displayName,
                )
              } else {
                void emitLlmModelDownloadFailed(
                  u.id,
                  u.provider_id ?? priorRow?.provider_id ?? '',
                  displayName,
                  u.error_message ?? priorRow?.error_message ?? '',
                )
              }
            }
            set((state) => {
              const updatedDownloads = state.downloads.map((download) => {
                const update = updates.find((u) => u.id === download.id)
                return update ? applyProgressUpdate(download, update) : download
              })
              const filteredDownloads = updatedDownloads.filter(
                (download) => download.status !== 'cancelled' && download.status !== 'completed',
              )
              return { downloads: filteredDownloads }
            })
          },
          complete: (_data: string) => {
            const allDownloads = get().downloads
            const providerIds = [
              ...new Set(allDownloads.map((d) => d.provider_id).filter((id): id is string => !!id)),
            ]
            for (const providerId of providerIds) {
              void useLlmProviderStore.getState().loadModelsForProvider(providerId)
            }
            void useLlmModelDownloadStore.getState().disconnectSSE()
            void loadExistingDownloads()
          },
          error: (errorMessage: string) => {
            console.error('SSE error:', errorMessage)
            set({ sseError: errorMessage, sseConnected: false })
          },
          default: (event: string, data: unknown) => {
            console.warn('Unknown SSE event:', event, data)
          },
        },
      })
    } catch (error) {
      console.error('SSE connection failed:', error)
      const attempts = get().reconnectAttempts + 1
      const maxAttempts = 5
      if (attempts < maxAttempts) {
        set({
          sseConnected: false,
          sseError: 'Connection lost, reconnecting...',
          reconnectAttempts: attempts,
        })
        setTimeout(() => {
          void action()
        }, 3000)
      } else {
        console.error('Max reconnection attempts reached')
        set({
          sseConnected: false,
          sseError: 'Failed to connect to download updates',
          reconnectAttempts: attempts,
        })
      }
    }
  }

  return action
}
