import type { ModelPickerGet, ModelPickerSet } from '../state'
import firstEnabledModelIdFactory from './_firstEnabledModelId'

export default (set: ModelPickerSet, get: ModelPickerGet) => {
  const firstEnabledModelId = firstEnabledModelIdFactory(get)
  return (key: string, conversationModelId?: string) => {
    const providers = get().providers
    // Prefer the conversation's own (enabled) model; else the first enabled.
    let resolved: string | null = null
    if (conversationModelId) {
      for (const provider of providers) {
        const match = provider.llm_models?.find(
          m => m.id === conversationModelId && m.enabled,
        )
        if (match) {
          resolved = match.id
          break
        }
      }
    }
    resolved = resolved ?? firstEnabledModelId()
    if (resolved) {
      set(state => {
        state.selectedByConversation[key] = resolved as string
      })
    }
  }
}
