import type { LlmProviderGet } from '../state'
import type { LlmProviderWithModels } from '../types'

export default (_set: unknown, get: LlmProviderGet) =>
  (id: string): LlmProviderWithModels | undefined =>
    get().providers.find(p => p.id === id)
