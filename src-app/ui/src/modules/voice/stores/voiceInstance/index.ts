import { defineStore, registerLazyStore } from '@ziee/framework/store-kit'
import { voiceInstanceState, type VoiceInstanceState } from './state'
import type { Actions } from './actions.gen'

const VoiceInstanceDef = defineStore<VoiceInstanceState, Actions>('VoiceInstance', {
  immer: true,
  state: voiceInstanceState,
  actions: import.meta.glob('./actions/*.ts'),
  init: ({ on, actions }) => {
    const reload = () => void actions.loadInstance()
    on('sync:reconnect', reload)
    void actions.loadInstance()
  },
})

export const VoiceInstance = registerLazyStore(VoiceInstanceDef)
export const useVoiceInstanceStore = VoiceInstanceDef.store
