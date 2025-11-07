import { Stores } from '@/core/stores'
import type { LlmRepository } from '@/api-client/types'

export const emitLlmRepositoryCreated = async (repository: LlmRepository) => {
  await Stores.EventBus.emit({
    type: 'llm_repository.created',
    data: { repository },
  })
}

export const emitLlmRepositoryUpdated = async (repository: LlmRepository) => {
  await Stores.EventBus.emit({
    type: 'llm_repository.updated',
    data: { repository },
  })
}

export const emitLlmRepositoryDeleted = async (repositoryId: string) => {
  await Stores.EventBus.emit({
    type: 'llm_repository.deleted',
    data: { repositoryId },
  })
}
