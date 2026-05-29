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

  clearError: () => set({ error: null })
}))
