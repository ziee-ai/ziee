import type { LlmProviderGet } from '../state'
import type { LlmProvider } from '@/api-client/types'

export default (_set: unknown, _get: LlmProviderGet) =>
  // API key is no longer required to enable a provider (users supply their own).
  (_provider: LlmProvider): boolean => true
