import type { VoiceInstanceSet } from '../state'

export default (set: VoiceInstanceSet) => () => {
  set({ error: null })
}
