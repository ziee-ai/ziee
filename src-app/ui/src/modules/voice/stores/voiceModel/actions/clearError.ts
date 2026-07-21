import type { VoiceModelGet, VoiceModelSet } from '../state'

export default (set: VoiceModelSet, _get: VoiceModelGet) =>
  () => {
    set({ error: null })
  }
