import type { LlmModel } from '@/api-client/types'
import { type LlmProvider as BaseLlmProvider } from '@/api-client/types'

// Extended type that includes models array.
// TODO: Backend should include llm_models in LlmProvider response.
export interface LlmProviderWithModels extends BaseLlmProvider {
  llm_models?: LlmModel[]
  // Whether an API key is configured (system- or user-level).
  api_key_configured?: boolean
}
