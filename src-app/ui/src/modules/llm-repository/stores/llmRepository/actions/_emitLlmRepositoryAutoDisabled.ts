import { Stores } from '@ziee/framework/stores'
import type { LlmRepositoryGet, LlmRepositorySet } from '../state'

/** Internal helper — _-prefixed so the glob skip it. Used by createLlmRepository + updateLlmRepository. */
export default (_set: LlmRepositorySet, _get: LlmRepositoryGet) => async (repositoryId: string, reason: string) => {
  await Stores.EventBus.emit({
    type: 'llm_repository.auto_disabled',
    data: { repositoryId, reason },
  })
}
