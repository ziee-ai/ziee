import { Stores } from '@ziee/framework/stores'
import type { LlmRepository } from '@/api-client/types'
import type { LlmRepositoryGet, LlmRepositorySet } from '../state'

/** Internal helper — _-prefixed so the glob skip it. Used by updateLlmRepository + testLlmRepositoryById. */
export default (_set: LlmRepositorySet, _get: LlmRepositoryGet) => async (repository: LlmRepository) => {
  await Stores.EventBus.emit({
    type: 'llm_repository.updated',
    data: { repository },
  })
}
