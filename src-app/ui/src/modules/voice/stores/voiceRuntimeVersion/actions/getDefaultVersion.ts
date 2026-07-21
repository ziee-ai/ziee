import type { RuntimeVersionResponse2 } from '@/api-client/types'
import type { VoiceRuntimeVersionGet, VoiceRuntimeVersionSet } from '../state'

export default (_set: VoiceRuntimeVersionSet, get: VoiceRuntimeVersionGet) =>
  (): RuntimeVersionResponse2 | null => {
    return get().versions.find(v => v.is_system_default) || null
  }
