import { registerSync } from '@/core/sync'
import { useUserLlmProvidersStore } from '@/modules/user-llm-providers/UserLlmProviders.store'

// A saved API key changed on another device (the event id is the provider
// id; only masked state is ever exposed). Reload the user's providers +
// masked-key map. `load()` refetches its own scoped, sanitized view.
registerSync('api_key', {
  onEvent: () => {
    void useUserLlmProvidersStore.getState().load()
  },
  onResync: () => {
    void useUserLlmProvidersStore.getState().load()
  },
})
