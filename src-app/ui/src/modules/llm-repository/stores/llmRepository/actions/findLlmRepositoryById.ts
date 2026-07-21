import type { LlmRepository } from '@/api-client/types'
import type { LlmRepositoryGet } from '../state'

export default (_set: unknown, get: LlmRepositoryGet) => (id: string): LlmRepository | undefined =>
  get().repositories.find(r => r.id === id)
