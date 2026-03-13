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

      // Compute additional fields
      const latestVersion = response.available_versions[0] || ''
      const hasUpdates = currentVersion
        ? response.available_versions.some((v: string) => v !== currentVersion.version)
        : response.available_versions.length > 0

      const updateCheck: RuntimeUpdateCheck = {
        engine: response.engine,
        available_versions: response.available_versions,
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
