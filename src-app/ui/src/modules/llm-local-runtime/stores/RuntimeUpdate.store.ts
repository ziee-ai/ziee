import { create } from 'zustand'
import { ApiClient } from '@/api-client'
import { Stores } from '@/core/stores'
import type { RuntimeEngine, RuntimeUpdateCheck } from '../types'

interface RuntimeUpdateState {
  // Data
  updateChecks: Map<RuntimeEngine, RuntimeUpdateCheck>

  // Loading states
  checking: Map<RuntimeEngine, boolean>
  error: string | null

  // Actions
  checkForUpdates: (engine: RuntimeEngine) => Promise<RuntimeUpdateCheck>
  clearError: () => void

  // Module-init hook (called automatically by the meta-framework on
  // module mount) — wires the cross-store event listeners.
  __init__?: {
    __store__: () => void
  }
  // Cleanup — removes the event listeners added by __init__.__store__
  // so they don't accumulate across mount/unmount cycles.
  __destroy__?: () => void
}

export const useRuntimeUpdateStore = create<RuntimeUpdateState>((set) => ({
  updateChecks: new Map(),
  checking: new Map(),
  error: null,

  checkForUpdates: async (engine: RuntimeEngine) => {
    set(state => ({
      checking: new Map(state.checking).set(engine, true),
      error: null
    }))

    try {
      const response = await ApiClient.RuntimeVersion.checkUpdates({
        engine
      })

      // Get current default version for this engine
      const currentVersion = Stores.RuntimeVersion.getDefaultVersion(engine)

      // Releases come newest-first. The "latest" we can actually install is
      // the newest one whose binary is published for this host.
      const versions = response.versions
      const latestVersion =
        versions.find(v => v.binary_ready)?.version || versions[0]?.version || ''
      // An update is available when there's a ready (built) version we have
      // not installed yet. Build-pending tags don't count.
      const hasUpdates = versions.some(v => v.binary_ready && !v.installed)

      const updateCheck: RuntimeUpdateCheck = {
        engine: response.engine,
        platform: response.platform,
        arch: response.arch,
        versions,
        current_version: currentVersion?.version,
        latest_version: latestVersion,
        has_updates: hasUpdates
      }

      set(state => {
        const newChecking = new Map(state.checking)
        newChecking.delete(engine)

        const newUpdateChecks = new Map(state.updateChecks)
        newUpdateChecks.set(engine, updateCheck)

        return {
          checking: newChecking,
          updateChecks: newUpdateChecks
        }
      })

      return updateCheck
    } catch (error) {
      set(state => {
        const newChecking = new Map(state.checking)
        newChecking.delete(engine)

        return {
          checking: newChecking,
          error: error instanceof Error ? error.message : 'Failed to check updates'
        }
      })
      throw error
    }
  },

  clearError: () => set({ error: null }),

  __init__: {
    __store__: () => {
      const eventBus = Stores.EventBus
      // When a version is deleted or created, the cached
      // updateChecks for its engine still flag it as installed/not
      // (stale), so the available-versions list misrenders the
      // "installed" tag + the Download button's disabled state.
      // Re-running the update check for every engine that has a
      // cached entry rebuilds the snapshot against the current DB
      // state. Cheap: there are only two engines.
      const refreshAllCached = () => {
        const checks = useRuntimeUpdateStore.getState().updateChecks
        for (const engine of checks.keys()) {
          useRuntimeUpdateStore
            .getState()
            .checkForUpdates(engine)
            .catch(() => {})
        }
      }
      eventBus.on(
        'runtime_version.deleted',
        refreshAllCached,
        'RuntimeUpdateStore',
      )
      eventBus.on(
        'runtime_version.created',
        refreshAllCached,
        'RuntimeUpdateStore',
      )
    },
  },
  __destroy__: () => {
    Stores.EventBus.removeGroupListeners('RuntimeUpdateStore')
  },
}))
