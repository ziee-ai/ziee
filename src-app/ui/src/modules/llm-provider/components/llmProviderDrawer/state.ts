import type { LlmProvider } from '@/api-client/types'
import type { StoreSet } from '@ziee/framework/store-kit'

export const llmProviderDrawerState = {
  isOpen: false,
  editingProvider: null as LlmProvider | null,
}

export type LlmProviderDrawerState = typeof llmProviderDrawerState
export type LlmProviderDrawerSet = StoreSet<LlmProviderDrawerState>
export type LlmProviderDrawerGet = () => LlmProviderDrawerState
