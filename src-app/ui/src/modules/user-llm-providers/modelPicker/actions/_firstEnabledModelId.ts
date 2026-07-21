import type { ModelPickerGet } from '../state'

/** Factory that returns a closure over the first enabled model ID. */
export default (get: ModelPickerGet): (() => string | null) => () => {
  for (const provider of get().providers) {
    const firstEnabled = provider.llm_models?.find(m => m.enabled)
    if (firstEnabled) return firstEnabled.id
  }
  return null
}
