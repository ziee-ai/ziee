import { Stores } from '@ziee/framework/stores'
import type { LlmRepositoryGet, LlmRepositorySet } from '../state'

/** Internal helper — _-prefixed so the glob skip it. Used by deleteLlmRepository. */
export default (_set: LlmRepositorySet, _get: LlmRepositoryGet) => async (repositoryId: string) => {
  await Stores.EventBus.emit({
    type: 'llm_repository.deleted',
    data: { repositoryId },
  })
}
