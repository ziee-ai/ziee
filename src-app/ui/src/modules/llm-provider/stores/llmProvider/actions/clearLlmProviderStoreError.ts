import type { LlmProviderSet } from '../state'

export default (set: LlmProviderSet) => () => {
  set({ error: null })
}
