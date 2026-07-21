import { defineStore, registerLazyStore } from '@ziee/framework/store-kit'
import { voiceModelDownloadProgressState, type VoiceModelDownloadProgressState } from './state'
import type { Actions } from './actions.gen'

const VoiceModelDownloadProgressDef = defineStore<VoiceModelDownloadProgressState, Actions>('VoiceModelDownloadProgress', {
  immer: true,
  state: voiceModelDownloadProgressState,
  actions: import.meta.glob('./actions/*.ts'),
  init: ({ actions }) => {
    void actions.loadActive()
  },
})

export const VoiceModelDownloadProgress = registerLazyStore(VoiceModelDownloadProgressDef)
export const useVoiceModelDownloadProgressStore = VoiceModelDownloadProgressDef.store
