/**
 * TEST-16 — VoiceModelUpload store (XHR FormData upload with progress).
 *
 * The `ApiClient.Voice.uploadModel` seam is mocked to expose the
 * `fileUploadProgress` callbacks so a test can drive `__init` / `onProgress` /
 * `onComplete` / `onError` by hand. Asserts:
 *   - `onProgress` updates the per-file percent/status + the overall percent
 *   - `onComplete` marks everything completed, clears `uploading`, refetches
 *   - `onError` records the error + flags the file errored
 *   - `cancelUpload()` aborts the in-flight XHR
 */
import { beforeEach, describe, expect, it, vi } from 'vitest'

type ProgressCbs = {
  __init: (xhr: { abort: () => void }) => void
  onProgress: (progress: number, fileIndex: number, overall: number) => void
  onComplete: () => void
  onError: (error: string, fileName?: string) => void
}

const h = vi.hoisted(() => ({
  cbs: null as ProgressCbs | null,
  xhr: { abort: vi.fn() },
  resolve: null as ((v: unknown) => void) | null,
  reject: null as ((e: unknown) => void) | null,
}))

const apiMock = vi.hoisted(() => ({ Voice: { uploadModel: vi.fn() } }))
const storesMock = vi.hoisted(() => ({
  VoiceModel: { loadInstalled: vi.fn(() => Promise.resolve()) },
}))

vi.mock('@/api-client', () => ({ ApiClient: apiMock }))
vi.mock('@ziee/framework/stores', () => ({
  Stores: storesMock,
  createStoreProxy: () => ({}),
}))
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

import { useVoiceModelUploadStore } from './voiceModelUpload'

const store = () => useVoiceModelUploadStore.getState()

beforeEach(() => {
  vi.clearAllMocks()
  h.cbs = null
  h.xhr = { abort: vi.fn() }
  h.resolve = null
  h.reject = null
  // Capture the progress callbacks; hold the promise open so the test drives it.
  apiMock.Voice.uploadModel.mockImplementation(
    (_fd: FormData, opts: { fileUploadProgress: ProgressCbs }) => {
      h.cbs = opts.fileUploadProgress
      h.cbs.__init(h.xhr)
      return new Promise((res, rej) => {
        h.resolve = res
        h.reject = rej
      })
    },
  )
  useVoiceModelUploadStore.setState({
    uploading: false,
    uploadProgress: [],
    overallUploadProgress: 0,
    uploadError: null,
  })
})

function file() {
  return new File(['ggml-bytes'], 'ggml-base.bin', {
    type: 'application/octet-stream',
  })
}

describe('VoiceModelUpload (TEST-16)', () => {
  it('onProgress updates per-file percent/status and the overall percent', async () => {
    const p = store().uploadModel({ name: 'my-base', file: file() })
    // Kick the mock (its impl runs synchronously up to the pending promise).
    await Promise.resolve()

    h.cbs?.onProgress(50, 0, 42)

    expect(store().uploadProgress[0].progress).toBe(50)
    expect(store().uploadProgress[0].status).toBe('uploading')
    expect(store().overallUploadProgress).toBe(42)

    // Settle so the awaited action doesn't dangle.
    h.cbs?.onComplete()
    h.resolve?.({ id: 'm1' })
    await p
  })

  it('onComplete marks everything completed, clears uploading, and refetches', async () => {
    const p = store().uploadModel({ name: 'my-base', file: file() })
    await Promise.resolve()

    h.cbs?.onComplete()
    h.resolve?.({ id: 'm1' })
    await p

    expect(store().uploadProgress[0].progress).toBe(100)
    expect(store().uploadProgress[0].status).toBe('completed')
    expect(store().overallUploadProgress).toBe(100)
    expect(store().uploading).toBe(false)
    expect(storesMock.VoiceModel.loadInstalled).toHaveBeenCalledTimes(1)
  })

  it('onError records the error and flags the file errored', async () => {
    const p = store().uploadModel({ name: 'my-base', file: file() })
    await Promise.resolve()

    h.cbs?.onError('disk full', 'ggml-base.bin')

    expect(store().uploadError).toBe('disk full')
    expect(store().uploadProgress[0].status).toBe('error')
    expect(store().uploading).toBe(false)

    // The action rethrows the rejected upload promise; swallow it.
    h.reject?.(new Error('disk full'))
    await expect(p).rejects.toThrow()
  })

  it('cancelUpload() aborts the in-flight XHR', async () => {
    const p = store().uploadModel({ name: 'my-base', file: file() })
    await Promise.resolve()

    store().cancelUpload()

    expect(h.xhr.abort).toHaveBeenCalledTimes(1)

    // Cleanup: resolve so the pending promise doesn't dangle.
    h.cbs?.onComplete()
    h.resolve?.({ id: 'm1' })
    await p
  })
})
