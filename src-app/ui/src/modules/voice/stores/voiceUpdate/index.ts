import { defineStore, registerLazyStore } from '@ziee/framework/store-kit'
import { voiceUpdateState, type VoiceUpdateState } from './state'
import type { Actions } from './actions.gen'

const VoiceUpdateDef = defineStore<VoiceUpdateState, Actions>('VoiceUpdate', {
  state: voiceUpdateState,
  actions: import.meta.glob('./actions/*.ts'),
})

export const VoiceUpdate = registerLazyStore(VoiceUpdateDef)
export const useVoiceUpdateStore = VoiceUpdateDef.store
