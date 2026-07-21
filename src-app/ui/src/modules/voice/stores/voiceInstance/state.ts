import type { StoreSet } from '@ziee/framework/store-kit'
import type { VoiceInstanceInfo } from '@/api-client/types'

export const voiceInstanceState = {
  info: null as VoiceInstanceInfo | null,
  loading: false,
  busy: false,
  error: null as string | null,
}

export type VoiceInstanceState = typeof voiceInstanceState
export type VoiceInstanceSet = StoreSet<VoiceInstanceState>
export type VoiceInstanceGet = () => VoiceInstanceState
