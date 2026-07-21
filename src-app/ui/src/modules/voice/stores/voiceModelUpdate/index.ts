import { defineStore, registerLazyStore } from '@ziee/framework/store-kit'
import { voiceModelUpdateState, type VoiceModelUpdateState } from './state'
import type { Actions } from './actions.gen'

const VoiceModelUpdateDef = defineStore<VoiceModelUpdateState, Actions>('VoiceModelUpdate', {
  immer: true,
  state: voiceModelUpdateState,
  actions: import.meta.glob('./actions/*.ts'),
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
    void reload()
  },
})

export const VoiceModelUpdate = registerLazyStore(VoiceModelUpdateDef)
export const useVoiceModelUpdateStore = VoiceModelUpdateDef.store
