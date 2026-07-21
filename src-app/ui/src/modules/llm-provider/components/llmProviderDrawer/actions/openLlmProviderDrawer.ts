import type { LlmProviderDrawerGet, LlmProviderDrawerSet } from '../state'
import type { LlmProvider } from '@/api-client/types'

export default (set: LlmProviderDrawerSet, _get: LlmProviderDrawerGet) =>
  async (provider?: LlmProvider) => {
    set({ isOpen: true, editingProvider: provider ?? null })
  }
