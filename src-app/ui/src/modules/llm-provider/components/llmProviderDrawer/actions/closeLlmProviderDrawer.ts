import type { LlmProviderDrawerGet, LlmProviderDrawerSet } from '../state'

export default (set: LlmProviderDrawerSet, _get: LlmProviderDrawerGet) =>
  async () => {
    set({ isOpen: false, editingProvider: null })
  }
