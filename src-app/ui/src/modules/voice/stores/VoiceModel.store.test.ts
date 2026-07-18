/**
 * TEST-15 — VoiceModel store (installed whisper-model library + readiness).
 *
 * Asserts:
 *   - `loadInstalled()` maps the API response into `installed`
 *   - `activate()` flips `is_active` to the activated row (others off)
 *   - `remove()` drops the deleted row from `installed`
 *   - both reads self-gate on `VoiceAdminRead` (no-403 rule) — a missing perm
 *     short-circuits before any request
 *   - a `sync:voice_model` frame refetches the installed set (init wiring)
 */
import { beforeEach, describe, expect, it, vi } from 'vitest'
import type { VoiceModel as VoiceModelRow } from '@/api-client/types'

const apiMock = vi.hoisted(() => ({
  Voice: {
    listModels: vi.fn(),
    getModelStatus: vi.fn(() =>
      Promise.resolve({ model: 'base', present: true }),
    ),
    activateModel: vi.fn(),
    deleteModel: vi.fn(),
  },
}))

// Controllable permission gate so we can exercise the self-gating branch.
const perm = vi.hoisted(() => ({ allow: true }))

// A minimal event bus so `init`'s `on('sync:voice_model', …)` registers into a
// registry the test can emit against.
const bus = vi.hoisted(() => {
  const map = new Map<string, Set<(p?: unknown) => void>>()
  return {
    on: (event: string, handler: (p?: unknown) => void) => {
      let s = map.get(event)
      if (!s) {
        s = new Set()
        map.set(event, s)
      }
      s.add(handler)
      return () => s?.delete(handler)
    },
    removeGroupListeners: () => {
      /* noop */
    },
    emit: (event: string, payload?: unknown) =>
      map.get(event)?.forEach(fn => fn(payload)),
    clear: () => map.clear(),
  }
})

vi.mock('@/api-client', () => ({ ApiClient: apiMock }))
vi.mock('@/core/permissions', () => ({ hasPermissionNow: () => perm.allow }))
vi.mock('@ziee/framework/stores', () => ({
  Stores: {
    VoiceModelUpdate: { checkForUpdates: vi.fn(() => Promise.resolve()) },
  },
  createStoreProxy: () => ({}),
}))
vi.mock('@ziee/framework/events', () => ({
  useEventBusStore: {
    getState: () => ({
      on: bus.on,
      removeGroupListeners: bus.removeGroupListeners,
    }),
  },
}))

import { useVoiceModelStore } from './VoiceModel.store'

const store = () => useVoiceModelStore.getState()

function model(over: Partial<VoiceModelRow> = {}): VoiceModelRow {
  return {
    id: 'm1',
    name: 'base',
    filename: 'ggml-base.bin',
    is_active: false,
    size_bytes: 1000,
    source: 'catalog',
    update_available: false,
    verified: true,
    created_at: '2026-07-11T00:00:00Z',
    ...over,
  } as VoiceModelRow
}

beforeEach(() => {
  vi.clearAllMocks()
  perm.allow = true
  bus.clear()
  useVoiceModelStore.setState({
    status: null,
    installed: [],
    loading: false,
    loadingInstalled: false,
    activating: new Map(),
    deleting: new Map(),
    error: null,
  })
})

describe('VoiceModel (TEST-15)', () => {
  it('loadInstalled() maps the response into installed', async () => {
    const rows = [model({ id: 'a' }), model({ id: 'b', name: 'small' })]
    apiMock.Voice.listModels.mockResolvedValueOnce(rows)

    await store().loadInstalled()

    expect(store().installed).toEqual(rows)
    expect(store().loadingInstalled).toBe(false)
  })

  it('loadInstalled() self-gates on VoiceAdminRead (no perm → no request)', async () => {
    perm.allow = false

    await store().loadInstalled()

    expect(apiMock.Voice.listModels).not.toHaveBeenCalled()
    expect(store().installed).toEqual([])
  })

  it('activate() flips is_active onto the activated row only', async () => {
    useVoiceModelStore.setState({
      installed: [model({ id: 'a', is_active: true }), model({ id: 'b' })],
    })
    apiMock.Voice.activateModel.mockResolvedValueOnce(
      model({ id: 'b', is_active: true }),
    )

    await store().activate('b')

    const byId = Object.fromEntries(
      store().installed.map(m => [m.id, m.is_active]),
    )
    expect(byId).toEqual({ a: false, b: true })
    expect(store().activating.has('b')).toBe(false)
  })

  it('remove() drops the deleted row from installed', async () => {
    useVoiceModelStore.setState({
      installed: [model({ id: 'a' }), model({ id: 'b' })],
    })
    apiMock.Voice.deleteModel.mockResolvedValueOnce(undefined)

    await store().remove('a')

    expect(store().installed.map(m => m.id)).toEqual(['b'])
    expect(store().deleting.has('a')).toBe(false)
    expect(apiMock.Voice.deleteModel).toHaveBeenCalledWith({
      id: 'a',
      ack_active: false,
    })
  })

  it('refetches the installed set on a sync:voice_model frame', async () => {
    apiMock.Voice.listModels.mockResolvedValue([model()])
    // Wire the store's init listeners (also fires the initial load).
    store().__init__.__store__()
    await Promise.resolve()
    apiMock.Voice.listModels.mockClear()

    bus.emit('sync:voice_model')
    await Promise.resolve()

    expect(apiMock.Voice.listModels).toHaveBeenCalledTimes(1)
  })
})
