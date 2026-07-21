import type { StoreSet } from '@ziee/framework/store-kit'
import type { VoiceCatalogModel } from '@/api-client/types'

export const voiceModelUpdateState = {
  catalog: [] as VoiceCatalogModel[],
  sourceReachable: true,
  sourceRepo: '' as string,
  hasLoaded: false,
  checking: false,
  error: null as string | null,
}

export type VoiceModelUpdateState = typeof voiceModelUpdateState
export type VoiceModelUpdateSet = StoreSet<VoiceModelUpdateState>
export type VoiceModelUpdateGet = () => VoiceModelUpdateState
