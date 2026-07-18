import { ApiClient } from '@/api-client'
import { defineStore } from '@ziee/framework/store-kit'
import { Stores } from '@ziee/framework/stores'
import type { RuntimeEngine, RuntimeUpdateCheck } from '../types'

export const RuntimeUpdate = defineStore('RuntimeUpdate', {
  state: {
    updateChecks: new Map<RuntimeEngine, RuntimeUpdateCheck>(),
    checking: new Map<RuntimeEngine, boolean>(),
    error: null as string | null,
  },
  actions: set => ({
    checkForUpdates: async (engine: RuntimeEngine): Promise<RuntimeUpdateCheck> => {
      set(state => ({
        checking: new Map(state.checking).set(engine, true),
        error: null,
      }))
      try {
        const response = await ApiClient.RuntimeVersion.checkUpdates({ engine })
        // Get current default version for this engine
        const currentVersion = Stores.RuntimeVersion.getDefaultVersion(engine)
        // Releases come newest-first. The "latest" we can actually install is the
        // newest one whose binary is published for this host.
        const versions = response.versions
        const latestVersion =
          versions.find(v => v.binary_ready)?.version || versions[0]?.version || ''
        // An update is available when there's a ready (built) version not yet
        // installed. Build-pending tags don't count.
        const hasUpdates = versions.some(v => v.binary_ready && !v.installed)
        const updateCheck: RuntimeUpdateCheck = {
          engine: response.engine,
          platform: response.platform,
          arch: response.arch,
          versions,
          current_version: currentVersion?.version,
          latest_version: latestVersion,
          has_updates: hasUpdates,
        }
        set(state => {
          const newChecking = new Map(state.checking)
          newChecking.delete(engine)
          const newUpdateChecks = new Map(state.updateChecks)
          newUpdateChecks.set(engine, updateCheck)
          return { checking: newChecking, updateChecks: newUpdateChecks }
        })
        return updateCheck
      } catch (error) {
        set(state => {
          const newChecking = new Map(state.checking)
          newChecking.delete(engine)
          return {
            checking: newChecking,
            error: error instanceof Error ? error.message : 'Failed to check updates',
          }
        })
        throw error
      }
    },
    clearError: () => set({ error: null }),
  }),
  init: ({ on, get, actions }) => {
    // When a version is deleted or created, the cached updateChecks for its
    // engine still flag it as installed/not (stale). Re-run the check for every
    // engine that has a cached entry to rebuild against current DB state. Cheap:
    // only two engines.
    const refreshAllCached = () => {
      for (const engine of get().updateChecks.keys()) {
        actions.checkForUpdates(engine).catch(() => {})
      }
    }
    on('runtime_version.deleted', refreshAllCached)
    on('runtime_version.created', refreshAllCached)
  },
})

export const useRuntimeUpdateStore = RuntimeUpdate.store
