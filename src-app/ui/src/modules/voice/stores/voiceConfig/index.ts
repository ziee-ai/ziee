import { defineStore, registerLazyStore } from '@ziee/framework/store-kit'
import { voiceConfigState, type VoiceConfigState } from './state'
import type { Actions } from './actions.gen'

const VoiceConfigDef = defineStore<VoiceConfigState, Actions>('VoiceConfig', {
  immer: true,
  state: voiceConfigState,
  actions: import.meta.glob('./actions/*.ts'),
  init: ({ on, actions }) => {
    const reload = () => void actions.loadSettings()
    on('sync:voice_settings', reload)
    on('sync:reconnect', reload)
    void actions.loadSettings()
  },
})

export const VoiceConfig = registerLazyStore(VoiceConfigDef)
export const useVoiceConfigStore = VoiceConfigDef.store
