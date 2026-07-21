import type { VoiceConfigSet } from '../state'

export default (set: VoiceConfigSet) => () => {
  set({ error: null })
}
