import type { StoreSet } from '@ziee/framework/store-kit'

export const voiceUploadModelDrawerState = {
  open: false,
}

export type VoiceUploadModelDrawerState = typeof voiceUploadModelDrawerState
export type VoiceUploadModelDrawerSet = StoreSet<VoiceUploadModelDrawerState>
export type VoiceUploadModelDrawerGet = () => VoiceUploadModelDrawerState
