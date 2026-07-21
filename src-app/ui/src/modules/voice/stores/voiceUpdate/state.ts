import type { StoreSet } from '@ziee/framework/store-kit'
import { type AvailableUpdatesResponse2 } from '@/api-client/types'

export const voiceUpdateState = {
  updateCheck: null as AvailableUpdatesResponse2 | null,
  checking: false,
  error: null as string | null,
}

export type VoiceUpdateState = typeof voiceUpdateState
export type VoiceUpdateSet = StoreSet<VoiceUpdateState>
export type VoiceUpdateGet = () => VoiceUpdateState
