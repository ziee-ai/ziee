/**
 * TEST-27 — VoiceModelUpdate store (downloadable whisper-model catalog).
 *
 * Asserts:
 *   - `checkForUpdates()` maps the catalog response (models + source repo)
 *   - a source-unreachable response (graceful degrade) sets
 *     `sourceReachable=false` with an empty catalog
 *   - a thrown fetch error records `error` and rejects (state otherwise intact)
 *   - the read self-gates on `VoiceAdminRead` (no-403 rule)
 */
import { beforeEach, describe, expect, it, vi } from 'vitest'
import type { VoiceCatalogModel } from '@/api-client/types'

const apiMock = vi.hoisted(() => ({ Voice: { listModelCatalog: vi.fn() } }))
const perm = vi.hoisted(() => ({ allow: true }))

vi.mock('@/api-client', () => ({ ApiClient: apiMock }))
vi.mock('@/core/permissions', () => ({ hasPermissionNow: () => perm.allow }))
vi.mock('@ziee/framework/stores', () => ({ Stores: {}, createStoreProxy: () => ({}) }))
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

import { useVoiceModelUpdateStore } from './VoiceModelUpdate.store'

const store = () => useVoiceModelUpdateStore.getState()

function catModel(over: Partial<VoiceCatalogModel> = {}): VoiceCatalogModel {
  return {
    name: 'large-v3',
    filename: 'ggml-large-v3.bin',
    english_only: false,
    installed: false,
    ...over,
  } as VoiceCatalogModel
}

beforeEach(() => {
  vi.clearAllMocks()
  perm.allow = true
  useVoiceModelUpdateStore.setState({
    catalog: [],
    sourceReachable: true,
    sourceRepo: '',
    hasLoaded: false,
    checking: false,
    error: null,
  })
})

describe('VoiceModelUpdate (TEST-27)', () => {
  it('checkForUpdates() maps the catalog response', async () => {
    const models = [
      catModel(),
      catModel({ name: 'base.en', english_only: true }),
    ]
    apiMock.Voice.listModelCatalog.mockResolvedValueOnce({
      models,
      source_reachable: true,
      source_repo: 'ggerganov/whisper.cpp',
    })

    const res = await store().checkForUpdates()

    expect(res?.models).toEqual(models)
    expect(store().catalog).toEqual(models)
    expect(store().sourceReachable).toBe(true)
    expect(store().sourceRepo).toBe('ggerganov/whisper.cpp')
    expect(store().hasLoaded).toBe(true)
    expect(store().checking).toBe(false)
  })

  it('a source-unreachable response degrades to sourceReachable=false + empty catalog', async () => {
    apiMock.Voice.listModelCatalog.mockResolvedValueOnce({
      models: [],
      source_reachable: false,
      source_repo: 'ggerganov/whisper.cpp',
    })

    await store().checkForUpdates()

    expect(store().sourceReachable).toBe(false)
    expect(store().catalog).toEqual([])
    expect(store().hasLoaded).toBe(true)
  })

  it('a fetch error records the error and rejects', async () => {
    apiMock.Voice.listModelCatalog.mockRejectedValueOnce(
      new Error('network down'),
    )

    await expect(store().checkForUpdates()).rejects.toThrow('network down')

    expect(store().error).toBe('network down')
    expect(store().checking).toBe(false)
  })

  it('self-gates on VoiceAdminRead (no perm → returns null, no request)', async () => {
    perm.allow = false

    const res = await store().checkForUpdates()

    expect(res).toBeNull()
    expect(apiMock.Voice.listModelCatalog).not.toHaveBeenCalled()
  })
})
