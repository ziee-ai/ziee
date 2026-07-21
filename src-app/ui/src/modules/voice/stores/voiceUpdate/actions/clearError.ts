import type { VoiceUpdateGet, VoiceUpdateSet } from '../state'

export default (set: VoiceUpdateSet, _get: VoiceUpdateGet) => () => {
  set({ error: null })
}
