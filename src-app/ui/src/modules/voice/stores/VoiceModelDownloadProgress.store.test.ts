/**
 * TEST-14 — VoiceModelDownloadProgress store (whisper model download progress).
 *
 * Drives the store headless (getState/setState) with the SSE seam
 * (`ApiClient.Voice.subscribeModelDownloadEvents`) mocked so we can hand-feed
 * `progress` / `complete` frames. Asserts:
 *   - a `progress` frame updates `activeByKey[key]` bytes + derived percent
 *   - a `complete` frame flips status→completed and triggers the installed
 *     library + catalog refetch, then dismisses the entry
 *   - `loadActive()` seeds every task and re-subscribes ONLY the non-terminal
 *     ones (a completed task is not re-attached)
 */
import { beforeEach, describe, expect, it, vi } from 'vitest'

// ── mocked seams ─────────────────────────────────────────────────────────────

/** Captured per subscribe call so a test can hand-feed SSE frames. */
type SseHandlers = {
  __init: (a: { abortController: AbortController }) => void
  connected: () => void
  progress: (d: {
    status: string
    bytes_received: number
    total_bytes?: number
    percent?: number
  }) => void
  complete: (d: { model_id: string; bytes_downloaded: number }) => void
  failed: (d: { error: string }) => void
}

const h = vi.hoisted(() => ({
  lastSse: null as SseHandlers | null,
  subscribeCalls: [] as string[],
}))

const apiMock = vi.hoisted(() => ({
  Voice: {
    listModelDownloads: vi.fn(),
    downloadModel: vi.fn(),
    cancelModelDownload: vi.fn(),
    subscribeModelDownloadEvents: vi.fn(),
  },
}))

const storesMock = vi.hoisted(() => ({
  VoiceModel: { loadInstalled: vi.fn(() => Promise.resolve()) },
  VoiceModelUpdate: { checkForUpdates: vi.fn(() => Promise.resolve()) },
}))

vi.mock('@/api-client', () => ({ ApiClient: apiMock }))
vi.mock('@ziee/framework/stores', () => ({
  Stores: storesMock,
  createStoreProxy: () => ({}),
}))
vi.mock('@/core/permissions', () => ({ hasPermissionNow: () => true }))
// store-kit imports the event bus; nothing here fires it, so a no-op is enough.
vi.mock('@ziee/framework/events', () => ({
  useEventBusStore: {
    getState: () => ({
      on: () => () => {
        /* unsub */
      },
      removeGroupListeners: () => {
        /* noop */
      },
    }),
  },
}))

import { useVoiceModelDownloadProgressStore } from './VoiceModelDownloadProgress.store'

const store = () => useVoiceModelDownloadProgressStore.getState()

/** Wire the subscribe mock to capture handlers + record the key. */
function wireSubscribe(): void {
  apiMock.Voice.subscribeModelDownloadEvents.mockImplementation(
    (params: { key: string }, opts: { SSE: SseHandlers }) => {
      h.subscribeCalls.push(params.key)
      h.lastSse = opts.SSE
      opts.SSE.__init({ abortController: new AbortController() })
      return Promise.resolve()
    },
  )
}

beforeEach(() => {
  vi.clearAllMocks()
  h.lastSse = null
  h.subscribeCalls = []
  wireSubscribe()
  useVoiceModelDownloadProgressStore.setState({
    activeByKey: new Map(),
    loadingActive: false,
    error: null,
  })
})

describe('VoiceModelDownloadProgress (TEST-14)', () => {
  it('a progress frame updates bytes + derived percent for the key', async () => {
    const key = 'progress-key'
    apiMock.Voice.downloadModel.mockResolvedValueOnce({
      task_id: 't1',
      key,
      name: 'large-v3',
      events_url: `/api/voice/models/downloads/${key}/events`,
    })

    await store().startDownload({ name: 'large-v3' })
    // Seeded at 0 bytes on start.
    expect(store().activeByKey.get(key)?.bytes_received).toBe(0)

    // A progress frame with no explicit percent → derived from bytes/total.
    h.lastSse?.progress({
      status: 'downloading',
      bytes_received: 500,
      total_bytes: 1000,
    })

    const entry = store().activeByKey.get(key)
    expect(entry?.status).toBe('downloading')
    expect(entry?.bytes_received).toBe(500)
    expect(entry?.percent).toBe(50)
  })

  it('a complete frame flips status→completed, refetches the library + catalog, then dismisses', async () => {
    vi.useFakeTimers()
    try {
      const key = 'complete-key'
      apiMock.Voice.downloadModel.mockResolvedValueOnce({
        task_id: 't2',
        key,
        name: 'base.en',
        events_url: `/api/voice/models/downloads/${key}/events`,
      })

      await store().startDownload({ name: 'base.en' })
      h.lastSse?.complete({ model_id: 'm-1', bytes_downloaded: 4096 })

      const entry = store().activeByKey.get(key)
      expect(entry?.status).toBe('completed')
      expect(entry?.percent).toBe(100)
      expect(entry?.bytes_received).toBe(4096)

      // Complete refreshes the installed library + the catalog (installed flags).
      expect(storesMock.VoiceModel.loadInstalled).toHaveBeenCalledTimes(1)
      expect(storesMock.VoiceModelUpdate.checkForUpdates).toHaveBeenCalledTimes(
        1,
      )

      // The entry auto-dismisses after the 2s grace.
      expect(store().activeByKey.has(key)).toBe(true)
      vi.advanceTimersByTime(2000)
      expect(store().activeByKey.has(key)).toBe(false)
    } finally {
      vi.useRealTimers()
    }
  })

  it('loadActive() seeds all tasks and re-subscribes only the non-terminal ones', async () => {
    apiMock.Voice.listModelDownloads.mockResolvedValueOnce([
      {
        task_id: 'a',
        key: 'active-1',
        name: 'small',
        status: 'downloading',
        bytes_received: 10,
        total_bytes: 100,
      },
      {
        task_id: 'b',
        key: 'done-1',
        name: 'tiny',
        status: 'completed',
        bytes_received: 100,
        total_bytes: 100,
      },
    ])

    await store().loadActive()

    // Both snapshots repaint immediately.
    expect(store().activeByKey.get('active-1')?.status).toBe('downloading')
    expect(store().activeByKey.get('done-1')?.status).toBe('completed')
    expect(store().loadingActive).toBe(false)

    // Only the in-flight task re-opens an SSE subscription.
    expect(h.subscribeCalls).toEqual(['active-1'])
  })
})
