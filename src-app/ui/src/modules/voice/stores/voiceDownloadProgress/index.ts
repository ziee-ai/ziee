import { defineStore, registerLazyStore } from '@ziee/framework/store-kit'
import { voiceDownloadProgressState, type VoiceDownloadProgressState } from './state'
import type { Actions } from './actions.gen'

const VoiceDownloadProgressDef = defineStore<VoiceDownloadProgressState, Actions>(
  'VoiceDownloadProgress',
  {
    immer: true,
    state: voiceDownloadProgressState,
    actions: import.meta.glob('./actions/*.ts'),
    init: ({ actions }) => {
      void actions.loadActive()
    },
  },
)

export const VoiceDownloadProgress = registerLazyStore(VoiceDownloadProgressDef)
export const useVoiceDownloadProgressStore = VoiceDownloadProgressDef.store
