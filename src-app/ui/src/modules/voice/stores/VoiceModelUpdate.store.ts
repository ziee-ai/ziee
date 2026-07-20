import { ApiClient } from '@/api-client'
import { type VoiceCatalogModel, type VoiceCatalogResponse } from '@/api-client/types'
import { Permissions } from '@/api-client/permissions'
import { hasPermissionNow } from '@/core/permissions'
import { defineStore } from '@ziee/framework/store-kit'

/**
 * The downloadable whisper-model catalog fetched from the configured source
 * repo, with a source-reachability signal for graceful degrade. Mirrors the
 * sibling `VoiceUpdate` (runtime release feed) but owns the model catalog.
 */
export const VoiceModelUpdate = defineStore('VoiceModelUpdate', {
  state: {
    catalog: [] as VoiceCatalogModel[],
    sourceReachable: true,
    sourceRepo: '' as string,
    hasLoaded: false,
    checking: false,
    error: null as string | null,
  },
  actions: set => ({
    checkForUpdates: async (): Promise<VoiceCatalogResponse | null> => {
      if (!hasPermissionNow(Permissions.VoiceAdminRead)) return null
      set({ checking: true, error: null })
      try {
        const response = await ApiClient.Voice.listModelCatalog()
        set({
          catalog: response.models,
          sourceReachable: response.source_reachable,
          sourceRepo: response.source_repo,
          hasLoaded: true,
          checking: false,
        })
        return response
      } catch (error) {
        set({
          checking: false,
          error:
            error instanceof Error
              ? error.message
              : 'Failed to load model catalog',
        })
        throw error
      }
    },
    clearError: () => set({ error: null }),
  }),
  init: ({ on, actions }) => {
    // The catalog is fetched from settings.model_source_repo — a settings change
    // (local or remote) can repoint the source, so re-fetch on that sync.
    // checkForUpdates self-gates on VoiceAdminRead (no-403 rule).
    const reload = () =>
      void actions.checkForUpdates().catch(() => {
        /* non-fatal */
      })
    on('sync:voice_settings', reload)
    on('sync:reconnect', reload)
    reload()
  },
})

export const useVoiceModelUpdateStore = VoiceModelUpdate.store
