import type { LlmRepositoryGet, LlmRepositorySet } from '../state'
import { EventBus } from '@ziee/framework/stores'

/** Internal helper — _-prefixed so the glob skip it. Used by deleteLlmRepository. */
export default (_set: LlmRepositorySet, _get: LlmRepositoryGet) => async (repositoryId: string) => {
  await EventBus.emit({
    type: 'llm_repository.deleted',
    data: { repositoryId },
  })
}
