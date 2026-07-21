import type { StoreSet } from '@ziee/framework/store-kit'
import type { LlmProvider } from '@/api-client/types'

/**
 * PRIVATE, per-widget store (one instance per group row). Each mounted widget
 * owns just ITS group's providers — no global Map, no shared cache.
 */
export const llmProviderGroupWidgetState = {
  groupId: '' as string,
  providers: [] as LlmProvider[],
  loading: false,
  error: null as string | null,
}

export type LlmProviderGroupWidgetState = typeof llmProviderGroupWidgetState
export type LlmProviderGroupWidgetSet = StoreSet<LlmProviderGroupWidgetState>
export type LlmProviderGroupWidgetGet = () => LlmProviderGroupWidgetState
