import type { LlmRepository } from '@/api-client/types'
import { EventBus } from '@ziee/framework/stores'

export const emitLlmRepositoryCreated = async (repository: LlmRepository) => {
  await EventBus.emit({
    type: 'llm_repository.created',
    data: { repository },
  })
}

export const emitLlmRepositoryUpdated = async (repository: LlmRepository) => {
  await EventBus.emit({
    type: 'llm_repository.updated',
    data: { repository },
  })
}

export const emitLlmRepositoryDeleted = async (repositoryId: string) => {
  await EventBus.emit({
    type: 'llm_repository.deleted',
    data: { repositoryId },
  })
}

/**
 * Emitted from the store's create / update flows when the backend's
 * `connection_warning` field is populated (probe failed and the row
 * was auto-downgraded to `enabled = false`). Triggers a list reload
 * so the row's `last_health_check_status` flips to 'unhealthy' in
 * the visible DOM without a manual refresh.
 */
export const emitLlmRepositoryAutoDisabled = async (
  repositoryId: string,
  reason: string,
) => {
  await EventBus.emit({
    type: 'llm_repository.auto_disabled',
    data: { repositoryId, reason },
  })
}
