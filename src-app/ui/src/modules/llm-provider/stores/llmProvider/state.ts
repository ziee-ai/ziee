import type { StoreSet } from '@ziee/framework/store-kit'
import type { DiscoveredModel } from '@/api-client/types'
import type { LlmProviderWithModels } from './types'

export const llmProviderState = {
  providers: [] as LlmProviderWithModels[],
  isInitialized: false,
  loading: false,
  creating: false,
  updating: false,
  deleting: false,
  llmModelsLoading: {} as Record<string, boolean>, // providerId -> loading
  modelError: {} as Record<string, string>, // providerId -> error message
  llmModelOperations: {} as Record<string, boolean>, // modelId -> operation in progress
  // Model discovery (picker) per provider: results + loading, keyed by providerId.
  discoveredModels: {} as Record<string, DiscoveredModel[]>,
  discoverNotes: {} as Record<string, string[]>,
  discoverLoading: {} as Record<string, boolean>,
  // "Refresh models" (deprecation reconcile) in-flight, keyed by providerId.
  refreshingModels: {} as Record<string, boolean>,
  error: null as string | null,
}

export type LlmProviderState = typeof llmProviderState
export type LlmProviderSet = StoreSet<LlmProviderState>
export type LlmProviderGet = () => LlmProviderState
