/**
 * The model-download SSE subscription's RECONNECT ACCOUNTING.
 *
 * Same defect, same shape as the code-sandbox rootfs stream (see
 * `sandboxRootfsVersions/subscribeToInstallProgress.store.test.ts`): the
 * api-client fires the `__init` SSE callback as soon as `fetch()` resolves and
 * BEFORE checking `response.ok`, so a failing status reached `__init` — which
 * marked the stream connected and zeroed `reconnectAttempts`. The catch then
 * took it 0 → 1, forever short of the 5-attempt cap, so the reconnect never
 * terminated. Covered here too because a fix applied to only one of two
 * identical call sites leaves the other silently broken.
 */
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

const subscribeDownloadProgress = vi.hoisted(() => vi.fn())

vi.mock('@/api-client', () => ({
  ApiClient: { LlmModel: { subscribeDownloadProgress } },
}))
// Boundary stubs: this action imports sibling stores + event emitters at module
// load. None participate in the reconnect accounting under test.
vi.mock('@/modules/llm-provider/stores/llmProvider', () => ({
  useLlmProviderStore: { getState: () => ({ loadModelsForProvider: async () => undefined }) },
}))
vi.mock('@/modules/llm-provider/stores/llmModelDownload', () => ({
  useLlmModelDownloadStore: { getState: () => ({ disconnectSSE: async () => undefined }) },
}))
vi.mock('@/modules/llm-provider/events/emitters', () => ({
  emitLlmModelDownloadCompleted: async () => undefined,
  emitLlmModelDownloadFailed: async () => undefined,
}))
vi.mock('./actions/_loadExistingDownloads', () => ({
  default: () => async () => undefined,
}))

import makeSubscribe, { applyProgressUpdate } from './actions/subscribeToDownloadProgress'
import type { DownloadInstance, DownloadProgressUpdate } from '@/api-client/types'

type State = { downloads: unknown[]; sseConnected: boolean; sseError: string | null; reconnectAttempts: number }

/** Minimal store harness matching the action's (set, get) contract. */
function harness() {
  const state: State = { downloads: [], sseConnected: false, sseError: null, reconnectAttempts: 0 }
  const get = (() => state) as never
  const set = ((patch: Partial<State> | ((s: State) => Partial<State> | void)) => {
    const next = typeof patch === 'function' ? patch(state) : patch
    if (next) Object.assign(state, next)
  }) as never
  return { state, get, set }
}

function transportFailingWith(status: number) {
  return async (_body: unknown, opts: { SSE: Record<string, (...a: never[]) => void> }) => {
    opts.SSE.__init?.({ abortController: new AbortController() } as never)
    const err = new Error(`HTTP error! status: ${status}`) as Error & { status: number }
    err.status = status
    throw err
  }
}

beforeEach(() => {
  vi.useFakeTimers()
  subscribeDownloadProgress.mockReset()
})
afterEach(() => {
  vi.clearAllTimers()
  vi.useRealTimers()
})

describe('subscribeToDownloadProgress reconnect accounting', () => {
  it('accumulates the attempt count across consecutive failures', async () => {
    subscribeDownloadProgress.mockImplementation(transportFailingWith(503))
    const { state, get, set } = harness()
    const subscribe = makeSubscribe(set, get)

    await subscribe()
    expect(state.reconnectAttempts).toBe(1)
    await subscribe()

    // Pre-fix this read 1 again: `__init` reset the counter on every failed
    // attempt, so the cap was unreachable and the retry loop never stopped.
    expect(state.reconnectAttempts).toBe(2)
  })

  it('does not report the stream as connected when the request failed', async () => {
    subscribeDownloadProgress.mockImplementation(transportFailingWith(502))
    const { state, get, set } = harness()

    await makeSubscribe(set, get)()

    expect(state.sseConnected).toBe(false)
  })

  it('marks connected and clears the backoff on the server handshake', async () => {
    // Negative control: the fix must not be "never mark connected". A real 200
    // stream still has to flip the flag, or the re-entry guard would let every
    // caller open a duplicate stream.
    subscribeDownloadProgress.mockImplementation(
      async (_b: unknown, opts: { SSE: Record<string, (...a: never[]) => void> }) => {
        opts.SSE.__init?.({ abortController: new AbortController() } as never)
        opts.SSE.connected?.({} as never)
      },
    )
    const { state, get, set } = harness()
    state.reconnectAttempts = 3

    await makeSubscribe(set, get)()

    expect(state.sseConnected).toBe(true)
    expect(state.reconnectAttempts).toBe(0)
  })
})

/**
 * TEST-8 — acceptance test for INV-3: "a download's progress, as RENDERED by the
 * UI, must advance while the transfer runs."
 *
 * The owner watched a 5.68 GB model download run to completion while the
 * onboarding bar sat at 0% and the LLM-providers view read "0 Bytes / 0 Bytes".
 * The record was correct throughout — queried live mid-transfer it held
 * `current 5637699037 / total 5680522464` with a real speed and ETA — and the
 * server was broadcasting. The CONSUMER was wrong: `{ ...download, ...update }`
 * grafted the wire event's FLAT fields on as strays and never touched
 * `progress_data`, which is the only thing `DownloadItem` / `DownloadProgress` /
 * `ModelHubCard` read.
 *
 * These assert what a VIEW would render, not what the server wrote. The previous
 * round asserted the write, and the write was never the broken half.
 */
describe('TEST-8: the progress a view renders advances', () => {
  const BASE: DownloadInstance = {
    id: 'd1',
    provider_id: 'p1',
    repository_id: 'r1',
    request_data: {},
    status: 'downloading',
    // What `listDownloads` returns for a just-started download — the zeros the
    // UI was stuck on.
    progress_data: {
      phase: 'created',
      current: 0,
      total: 0,
      message: '',
      speed_bps: 0,
      eta_seconds: 0,
    },
    error_message: null,
    started_at: '2026-08-24T18:29:00Z',
    completed_at: null,
    model_id: null,
    created_at: '2026-08-24T18:29:00Z',
    updated_at: '2026-08-24T18:29:00Z',
  } as unknown as DownloadInstance

  /**
   * A flat wire frame.
   *
   * `phase` is REQUIRED on the wire and `error_message`/`model_id` are always
   * present (as explicit nulls when unset) — both pinned server-side by TEST-9 —
   * so every fixture here carries them.
   *
   * HONEST LIMIT (audit round 3): the all-null FIGURE fixture used below for the
   * progress-less case is NOT something the current server emits. Its INSERT
   * seeds a fully-zeroed `progress_data`, so a real queued row arrives with
   * `current: 0`, not null. That case therefore exercises the schema's
   * `progress_data: null` shape rather than an observed one, and the guard it
   * covers is defensive — it is not the reason a queued download renders
   * "0 Bytes / 0 Bytes", which comes from those seeded zeros.
   *
   * The `as unknown as` cast is needed because the generated type declares
   * `error_message?: string` while the wire sends `null` — a real codegen gap
   * (CODING_GUIDELINES §10 wants `| null`), noted rather than papered over, so
   * `tsc` does NOT pin this fixture's shape.
   */
  function frame(over: Partial<DownloadProgressUpdate>): DownloadProgressUpdate {
    return {
      id: 'd1',
      provider_id: 'p1',
      status: 'downloading',
      phase: 'downloading',
      // The real wire always carries these keys (TEST-9 pins that an absent
      // value serialises as null, not as a missing field), so the fixture does
      // too — otherwise the whole-row semantics below would be untested.
      error_message: null,
      model_id: null,
      ...over,
    } as unknown as DownloadProgressUpdate
  }

  it('lifts the flat wire fields into progress_data, and keeps advancing', () => {
    const first = applyProgressUpdate(
      BASE,
      frame({ current: 5_147_144_752, total: 5_680_522_464, speed_bps: 516_096, eta_seconds: 1033 }),
    )
    // The literal symptom: this used to stay 0 / 0.
    expect(first.progress_data?.current).toBe(5_147_144_752)
    expect(first.progress_data?.total).toBe(5_680_522_464)
    expect(first.progress_data?.speed_bps).toBe(516_096)
    expect(first.progress_data?.phase).toBe('downloading')

    const second = applyProgressUpdate(
      first,
      frame({ current: 5_637_699_037, total: 5_680_522_464, speed_bps: 1_606_723, eta_seconds: 26 }),
    )
    expect(second.progress_data!.current).toBeGreaterThan(first.progress_data!.current)

    // …and what the views actually compute from it.
    const percent = Math.round(
      (second.progress_data!.current / second.progress_data!.total) * 100,
    )
    expect(percent).toBe(99)
  })

  it('a null field does NOT blank a figure already on screen', () => {
    const shown = applyProgressUpdate(
      BASE,
      frame({ current: 5_147_144_752, total: 5_680_522_464, speed_bps: 516_096 }),
    )
    // The server sends these as Option; a frame that omits speed must not reset
    // the byte counts the user is watching to zero.
    const next = applyProgressUpdate(shown, frame({ current: 5_200_000_000, total: 5_680_522_464 }))
    expect(next.progress_data?.total).toBe(5_680_522_464)
    expect(next.progress_data?.speed_bps).toBe(516_096)
  })

  it('does not leave the flat wire keys on the row', () => {
    const merged = applyProgressUpdate(BASE, frame({ current: 1, total: 2 }))
    // The old spread put `current`/`total` at the top level, where nothing reads
    // them — the shape mismatch that hid behind `as DownloadInstance`.
    expect(merged).not.toHaveProperty('current')
    expect(merged).not.toHaveProperty('total')
    expect(merged).not.toHaveProperty('speed_bps')
  })

  it('a row with NO progress yet stays without progress_data', () => {
    // `DownloadItem.renderProgressInfo()` returns null when progress_data is
    // absent, so materialising a zeroed object for a row that has none would put
    // a byte line on screen where there was none. NOTE this is NOT the reason a
    // queued download shows "0 Bytes / 0 Bytes" — see the fixture note above;
    // that comes from the INSERT seeding zeros, and the claim that this guard
    // removed it was withdrawn.
    //
    // The frame below carries `phase: 'created'` (required on the wire, filled
    // in by `From<&DownloadInstance>` even with no progress_data) and every
    // optional figure null — the shape a row with `progress_data: NULL` would
    // produce. See the fixture note above for why the CURRENT server does not
    // actually emit it, and what that means for this test's scope.
    const queued = { ...BASE, progress_data: undefined } as unknown as DownloadInstance
    const merged = applyProgressUpdate(
      queued,
      frame({
        status: 'pending',
        phase: 'created',
        current: null,
        total: null,
        speed_bps: null,
        eta_seconds: null,
        message: null,
      } as unknown as Partial<DownloadProgressUpdate>),
    )
    expect(merged.progress_data).toBeUndefined()
  })

  it('a prototype-chain name is NOT accepted as a status', () => {
    // `wire in obj` answers true for 'toString'/'constructor'/'valueOf', which
    // would be written onto the row as a bogus status. Membership is tested
    // against a Set for exactly this reason (audit round 2).
    for (const bogus of ['toString', 'constructor', 'valueOf', 'hasOwnProperty']) {
      const merged = applyProgressUpdate(BASE, frame({ status: bogus as never }))
      expect(merged.status).toBe('downloading')
    }
  })

  it('a server-side CLEAR of error_message is observable', () => {
    // These whole-row fields are the opposite of the progress figures: the frame
    // carries the row's current value every time, so a null means "cleared", not
    // "unknown". Falling back to the previous value left stale red error text on
    // a row whose error the server had cleared (audit FIX-5).
    const failed = { ...BASE, error_message: 'network unreachable' } as DownloadInstance
    const recovered = applyProgressUpdate(
      failed,
      frame({ current: 10, total: 100 }),
    )
    expect(recovered.error_message).toBeNull()
  })

  it('an unrecognised wire status leaves the row alone', () => {
    const merged = applyProgressUpdate(
      BASE,
      frame({ status: 'not-a-real-status' as never }),
    )
    expect(merged.status).toBe('downloading')
  })

  it('still carries the terminal status through (the half that always worked)', () => {
    const done = applyProgressUpdate(BASE, frame({ status: 'completed', current: 5, total: 5 }))
    expect(done.status).toBe('completed')
  })
})
