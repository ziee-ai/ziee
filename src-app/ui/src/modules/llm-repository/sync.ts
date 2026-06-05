import { Permissions } from '@/api-client/types'
import { registerSync } from '@/core/sync'
import { useLlmRepositoryStore } from '@/modules/llm-repository/stores/LlmRepository.store'

// Admin LLM-repository table. `loadLlmRepositories` only skips while a load
// is in flight, so a plain reload refetches the current page.
const reload = () => {
  void useLlmRepositoryStore.getState().loadLlmRepositories()
}

registerSync('llm_repository', {
  onEvent: reload,
  onResync: reload,
  requiredPermission: Permissions.LlmRepositoriesRead,
})
