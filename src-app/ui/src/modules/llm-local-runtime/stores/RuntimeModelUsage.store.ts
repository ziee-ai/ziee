import { create } from 'zustand'
import { ApiClient } from '@/api-client'
import { Stores } from '@/core/stores'
import { hasPermissionNow } from '@/core/permissions'
import {
  Permissions,
  type InstanceResponse,
  type VersionUsageResponse,
} from '@/api-client/types'
import type { RuntimeEngine } from '../types'
import { emitRuntimeModelUsageChanged } from '../events/emitters'

interface RuntimeModelUsageState {
  // Per-engine usage snapshot (versions + the models that resolve to each).
  usage: Map<RuntimeEngine, VersionUsageResponse>
  // Per-engine load-in-flight.
  loading: Map<RuntimeEngine, boolean>
  // Per-model action-in-flight (start/stop/restart/swap), keyed by model id.
  acting: Map<string, boolean>
  // Per-model running-instance detail (port/base_url/health/error), lazily
  // loaded when a row is expanded. `null` = fetched, no instance.
  instances: Map<string, InstanceResponse | null>
  error: string | null

  loadUsage: (engine: RuntimeEngine) => Promise<void>
  startModel: (engine: RuntimeEngine, modelId: string) => Promise<void>
  stopModel: (engine: RuntimeEngine, modelId: string) => Promise<void>
  restartModel: (engine: RuntimeEngine, modelId: string) => Promise<void>
  swapVersion: (
    engine: RuntimeEngine,
    modelId: string,
    versionId: string
  ) => Promise<void>
  loadInstance: (modelId: string) => Promise<void>
  clearError: () => void

  __init__: { __store__: () => void }
  __destroy__: () => void
}

export const useRuntimeModelUsageStore = create<RuntimeModelUsageState>(
  (set, get) => ({
    usage: new Map(),
    loading: new Map(),
    acting: new Map(),
    instances: new Map(),
    error: null,

    loadUsage: async (engine: RuntimeEngine) => {
      set(state => ({
        loading: new Map(state.loading).set(engine, true),
        error: null
      }))
      try {
        const response = await ApiClient.RuntimeVersion.usage({ engine })
        set(state => {
          const loading = new Map(state.loading)
          loading.delete(engine)
          return {
            loading,
            usage: new Map(state.usage).set(engine, response)
          }
        })
      } catch (error) {
        set(state => {
          const loading = new Map(state.loading)
          loading.delete(engine)
          return {
            loading,
            error: error instanceof Error ? error.message : 'Failed to load usage'
          }
        })
      }
    },

    startModel: async (engine, modelId) => {
      await act(set, modelId, () =>
        ApiClient.LocalRuntime.startModel({ model_id: modelId })
      )
      await get().loadUsage(engine)
      await emitRuntimeModelUsageChanged(modelId)
    },

    stopModel: async (engine, modelId) => {
      await act(set, modelId, () =>
        ApiClient.LocalRuntime.stopModel({ model_id: modelId })
      )
      await get().loadUsage(engine)
      await emitRuntimeModelUsageChanged(modelId)
    },

    restartModel: async (engine, modelId) => {
      await act(set, modelId, () =>
        ApiClient.LocalRuntime.restartModel({ model_id: modelId })
      )
      await get().loadUsage(engine)
      await get().loadInstance(modelId)
      await emitRuntimeModelUsageChanged(modelId)
    },

    swapVersion: async (engine, modelId, versionId) => {
      await act(set, modelId, () =>
        ApiClient.LocalRuntime.swapModelVersion({
          model_id: modelId,
          version_id: versionId
        })
      )
      await get().loadUsage(engine)
      await emitRuntimeModelUsageChanged(modelId)
    },

    loadInstance: async (modelId) => {
      try {
        const instance = await ApiClient.LocalRuntime.getInstance({
          model_id: modelId
        })
        set(state => ({
          instances: new Map(state.instances).set(modelId, instance)
        }))
      } catch {
        // 404 = no instance (never started / already reaped).
        set(state => ({
          instances: new Map(state.instances).set(modelId, null)
        }))
      }
    },

    clearError: () => set({ error: null }),

    // Re-resolve usage when versions change elsewhere: a download adds a
    // version, a delete removes one, and a default change alters which
    // version unpinned models effectively resolve to.
    __init__: {
      __store__: () => {
        const reload = () => {
          // Self-gate so a non-runtime-admin never 403s on a
          // `sync:reconnect` (which fires for every store).
          if (!hasPermissionNow(Permissions.RuntimeVersionRead)) return
          for (const engine of get().usage.keys()) {
            get().loadUsage(engine)
          }
        }
        const bus = Stores.EventBus
        bus.on('runtime_version.created', reload, 'RuntimeModelUsageStore')
        bus.on('runtime_version.deleted', reload, 'RuntimeModelUsageStore')
        bus.on('runtime_version.default_changed', reload, 'RuntimeModelUsageStore')
        // Cross-device: RuntimeVersion.store refetches versions on
        // `sync:runtime_version` but does NOT re-emit the local
        // `runtime_version.*` events, so this usage view would otherwise
        // stay stale after a remote version change. Subscribe directly.
        bus.on('sync:runtime_version', reload, 'RuntimeModelUsageStore')
        bus.on('sync:reconnect', reload, 'RuntimeModelUsageStore')
      }
    },

    __destroy__: () => {
      Stores.EventBus.removeGroupListeners('RuntimeModelUsageStore')
    }
  })
)

// Run a per-model action with the `acting` flag set + error capture.
type SetState = (
  partial:
    | Partial<RuntimeModelUsageState>
    | ((s: RuntimeModelUsageState) => Partial<RuntimeModelUsageState>)
) => void

async function act(
  set: SetState,
  modelId: string,
  fn: () => Promise<unknown>
) {
  set(state => ({
    acting: new Map(state.acting).set(modelId, true),
    error: null
  }))
  try {
    await fn()
  } catch (error) {
    set({ error: error instanceof Error ? error.message : 'Action failed' })
    throw error
  } finally {
    set(state => {
      const acting = new Map(state.acting)
      acting.delete(modelId)
      return { acting }
    })
  }
}
