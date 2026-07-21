import { defineStore, registerLazyStore } from '@ziee/framework/store-kit'
import { voiceUploadModelDrawerState, type VoiceUploadModelDrawerState } from './state'
import type { Actions } from './actions.gen'

const VoiceUploadModelDrawerDef = defineStore<VoiceUploadModelDrawerState, Actions>('VoiceUploadModelDrawer', {
  immer: true,
  state: voiceUploadModelDrawerState,
  actions: import.meta.glob('./actions/*.ts'),
})
export const VoiceUploadModelDrawer = registerLazyStore(VoiceUploadModelDrawerDef)
export const useVoiceUploadModelDrawerStore = VoiceUploadModelDrawerDef.store
