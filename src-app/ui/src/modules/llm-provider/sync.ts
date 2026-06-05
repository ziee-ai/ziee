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

registerSync('llm_provider', {
  onEvent: reloadAdminProviders,
  onResync: reloadAdminProviders,
})

registerSync('llm_model', {
  onEvent: reloadAdminProviders,
  onResync: reloadAdminProviders,
})
