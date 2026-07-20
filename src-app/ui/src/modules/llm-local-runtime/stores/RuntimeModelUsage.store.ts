import { ApiClient } from '@/api-client'
import { type InstanceResponse, type VersionUsageResponse } from '@/api-client/types'
import { Permissions } from '@/api-client/permissions'
import { hasPermissionNow } from '@/core/permissions'
import { defineStore } from '@ziee/framework/store-kit'
import type { StoreSet } from '@ziee/framework/store-kit'
import type { RuntimeEngine } from '../types'
import { emitRuntimeModelUsageChanged } from '../events/emitters'

interface RuntimeModelUsageState {
  usage: Map<RuntimeEngine, VersionUsageResponse>
  loading: Map<RuntimeEngine, boolean>
  acting: Map<string, boolean>
  instances: Map<string, InstanceResponse | null>
  error: string | null
}

// Run a per-model action with the `acting` flag set + error capture.
async function act(
  set: StoreSet<RuntimeModelUsageState>,
  modelId: string,
  fn: () => Promise<unknown>,
) {
  set(state => ({
    acting: new Map(state.acting).set(modelId, true),
    error: null,
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

export const RuntimeModelUsage = defineStore('RuntimeModelUsage', {
  state: {
    // Per-engine usage snapshot (versions + the models that resolve to each).
    usage: new Map<RuntimeEngine, VersionUsageResponse>(),
    // Per-engine load-in-flight.
    loading: new Map<RuntimeEngine, boolean>(),
    // Per-model action-in-flight (start/stop/restart/swap), keyed by model id.
    acting: new Map<string, boolean>(),
    // Per-model running-instance detail, lazily loaded. `null` = fetched, none.
    instances: new Map<string, InstanceResponse | null>(),
    error: null as string | null,
  },
  actions: set => {
    const loadUsage = async (engine: RuntimeEngine) => {
      set(state => ({
        loading: new Map(state.loading).set(engine, true),
        error: null,
      }))
      try {
        const response = await ApiClient.RuntimeVersion.usage({ engine })
        set(state => {
          const loading = new Map(state.loading)
          loading.delete(engine)
          return { loading, usage: new Map(state.usage).set(engine, response) }
        })
      } catch (error) {
        set(state => {
          const loading = new Map(state.loading)
          loading.delete(engine)
          return {
            loading,
            error: error instanceof Error ? error.message : 'Failed to load usage',
          }
        })
      }
    }
    const loadInstance = async (modelId: string) => {
      try {
        const instance = await ApiClient.LocalRuntime.getInstance({ model_id: modelId })
        set(state => ({ instances: new Map(state.instances).set(modelId, instance) }))
      } catch {
        // 404 = no instance (never started / already reaped).
        set(state => ({ instances: new Map(state.instances).set(modelId, null) }))
      }
    }
    return {
      loadUsage,
      loadInstance,
      startModel: async (engine: RuntimeEngine, modelId: string) => {
        await act(set, modelId, () => ApiClient.LocalRuntime.startModel({ model_id: modelId }))
        await loadUsage(engine)
        await emitRuntimeModelUsageChanged(modelId)
      },
      stopModel: async (engine: RuntimeEngine, modelId: string) => {
        await act(set, modelId, () => ApiClient.LocalRuntime.stopModel({ model_id: modelId }))
        await loadUsage(engine)
        await emitRuntimeModelUsageChanged(modelId)
      },
      restartModel: async (engine: RuntimeEngine, modelId: string) => {
        await act(set, modelId, () =>
          ApiClient.LocalRuntime.restartModel({ model_id: modelId }),
        )
        await loadUsage(engine)
        await loadInstance(modelId)
        await emitRuntimeModelUsageChanged(modelId)
      },
      swapVersion: async (engine: RuntimeEngine, modelId: string, versionId: string) => {
        await act(set, modelId, () =>
          ApiClient.LocalRuntime.swapModelVersion({ model_id: modelId, version_id: versionId }),
        )
        await loadUsage(engine)
        await emitRuntimeModelUsageChanged(modelId)
      },
      clearError: () => set({ error: null }),
    }
  },
  // Re-resolve usage when versions change elsewhere: a download adds a version,
  // a delete removes one, and a default change alters which version unpinned
  // models resolve to.
  init: ({ on, get, actions }) => {
    const reload = () => {
      // Self-gate so a non-runtime-admin never 403s on a reconnect.
      if (!hasPermissionNow(Permissions.RuntimeVersionRead)) return
      for (const engine of get().usage.keys()) void actions.loadUsage(engine)
    }
    on('runtime_version.created', reload)
    on('runtime_version.deleted', reload)
    on('runtime_version.default_changed', reload)
    // Cross-device: RuntimeVersion.store refetches on sync:runtime_version but
    // does NOT re-emit local runtime_version.* events, so subscribe directly.
    on('sync:runtime_version', reload)
    on('sync:reconnect', reload)
  },
})

export const useRuntimeModelUsageStore = RuntimeModelUsage.store
