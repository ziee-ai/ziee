import type { StoreSet } from '@ziee/framework/store-kit'
import type { LlmModel } from '@/api-client/types'

// Picks the small subset of `LlmModel` the embedding-model dropdown needs.
export type EmbeddingCapableModel = Pick<
  LlmModel,
  'id' | 'name' | 'display_name' | 'provider_id'
>

export const memorySetupStepState = {
  enableMemory: false,
  embeddingModelId: null as string | null,
  availableModels: [] as EmbeddingCapableModel[],
  loading: false,
  saving: false,
  error: null as string | null,
}

export type MemorySetupStepState = typeof memorySetupStepState
export type MemorySetupStepSet = StoreSet<MemorySetupStepState>
export type MemorySetupStepGet = () => MemorySetupStepState
