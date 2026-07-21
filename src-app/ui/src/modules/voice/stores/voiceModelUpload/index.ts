import { defineStore, registerLazyStore } from '@ziee/framework/store-kit'
import {
  voiceModelUploadState,
  type VoiceModelUploadState,
} from './state'
import type { Actions } from './actions.gen'

const VoiceModelUploadDef = defineStore<VoiceModelUploadState, Actions>(
  'VoiceModelUpload',
  {
    immer: true,
    state: voiceModelUploadState,
    actions: import.meta.glob('./actions/*.ts'),
  },
)
export const VoiceModelUpload = registerLazyStore(VoiceModelUploadDef)
export const useVoiceModelUploadStore = VoiceModelUploadDef.store
