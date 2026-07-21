import { defineStore, registerLazyStore } from '@ziee/framework/store-kit'
import { voiceModelState, type VoiceModelState } from './state'
import type { Actions } from './actions.gen'

const VoiceModelDef = defineStore<VoiceModelState, Actions>('VoiceModel', {
  state: voiceModelState,
  actions: import.meta.glob('./actions/*.ts'),
  init: ({ on, actions }) => {
    const reloadModels = () => void actions.loadInstalled()
    const reloadStatus = () => void actions.loadStatus()
    on('sync:voice_model', reloadModels)
    on('sync:voice_settings', reloadStatus)
    on('sync:reconnect', () => {
      reloadModels()
      reloadStatus()
    })
    void actions.loadInstalled()
    void actions.loadStatus()
  },
})

export const VoiceModel = registerLazyStore(VoiceModelDef)
export const useVoiceModelStore = VoiceModelDef.store
