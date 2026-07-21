import type { VoiceModelUpdateGet, VoiceModelUpdateSet } from '../state'

export default (set: VoiceModelUpdateSet, _get: VoiceModelUpdateGet) =>
  () => {
    set(s => {
      s.error = null
    })
  }
