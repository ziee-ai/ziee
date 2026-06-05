import { Permissions } from '@/api-client/types'
import { registerSync } from '@/core/sync'
import { useLlmProviderStore } from '@/modules/llm-provider/stores/LlmProvider.store'

// Admin LLM-provider table. The store loads providers WITH their models in
// one pass, so a single reload covers both `llm_provider` and `llm_model`
// admin notifications. `loadLlmProviders` early-returns once initialized,
// so reset the flag to force a refetch.
const reloadAdminProviders = () => {
  useLlmProviderStore.setState({ isInitialized: false })
  void useLlmProviderStore.getState().loadLlmProviders()
}

// `loadLlmProviders` fetches providers AND each provider's models, so the
// reload needs BOTH reads — gate on both to avoid a sub-admin holding only
// one of the two perms 403-ing on the other endpoint during resync.
const adminProvidersPerms = {
  allOf: [Permissions.LlmProvidersRead, Permissions.LlmModelsRead],
}

registerSync('llm_provider', {
  onEvent: reloadAdminProviders,
  onResync: reloadAdminProviders,
  requiredPermission: adminProvidersPerms,
})

registerSync('llm_model', {
  onEvent: reloadAdminProviders,
  onResync: reloadAdminProviders,
  requiredPermission: adminProvidersPerms,
})
