import { Permissions } from '@/api-client/permissions'
import { hasPermissionNow } from '@/core/permissions'
import { defineStore, registerLazyStore } from '@ziee/framework/store-kit'
import { Stores } from '@ziee/framework/stores'
import { voiceRuntimeVersionState, type VoiceRuntimeVersionState } from './state'
import type { Actions } from './actions.gen'

const VoiceRuntimeVersionDef = defineStore<VoiceRuntimeVersionState, Actions>('VoiceRuntimeVersion', {
  state: voiceRuntimeVersionState,
  actions: import.meta.glob('./actions/*.ts'),
  init: ({ on, actions }) => {
    // Cross-device sync: reload on a remote download/delete/default change or an
    // SSE reconnect. loadVersions self-gates on VoiceAdminRead (no-403 rule).
    const reload = () => void actions.loadVersions()
    on('sync:voice_runtime_version', reload)
    on('sync:reconnect', reload)
    void actions.loadVersions()
    // Keep the update-check fresh when the installed set is invalidated.
    on('sync:voice_runtime_version', () => {
      if (hasPermissionNow(Permissions.VoiceAdminRead)) {
        Stores.VoiceUpdate.checkForUpdates().catch(() => {})
      }
    })
  },
})

export const VoiceRuntimeVersion = registerLazyStore(VoiceRuntimeVersionDef)
export const useVoiceRuntimeVersionStore = VoiceRuntimeVersionDef.store
