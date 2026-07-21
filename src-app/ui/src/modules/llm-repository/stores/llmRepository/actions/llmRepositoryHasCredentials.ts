import type { LlmRepository } from '@/api-client/types'
import type { LlmRepositoryGet } from '../state'

export default (_set: unknown, _get: LlmRepositoryGet) => (repository: LlmRepository): boolean => {
  // Secrets (api_key/password/token) are write-only — never returned. The
  // server refuses to persist an api_key auth_type with an empty key, so a
  // row with auth_type != 'none' has credentials set.
  if (repository.auth_type === 'none') return true
  return true
}
