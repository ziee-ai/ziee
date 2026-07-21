import type { StoreSet } from '@ziee/framework/store-kit'
import type { VoiceSettings } from '@/api-client/types'

export const voiceConfigState = {
  settings: null as VoiceSettings | null,
  loadingSettings: false,
  savingSettings: false,
  error: null as string | null,
}

export type VoiceConfigState = typeof voiceConfigState
export type VoiceConfigSet = StoreSet<VoiceConfigState>
export type VoiceConfigGet = () => VoiceConfigState
