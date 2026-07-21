import type { LlmRepository } from '@/api-client/types'
import type { LlmRepositoryGet, LlmRepositorySet } from '../state'
import { EventBus } from '@ziee/framework/stores'

/** Internal helper — _-prefixed so the glob skip it. Used by createLlmRepository. */
export default (_set: LlmRepositorySet, _get: LlmRepositoryGet) => async (repository: LlmRepository) => {
  await EventBus.emit({
    type: 'llm_repository.created',
    data: { repository },
  })
}
