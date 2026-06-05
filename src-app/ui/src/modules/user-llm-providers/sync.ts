import { Permissions } from '@/api-client/types'
import { registerSync } from '@/core/sync'
import { useModelPickerStore } from '@/modules/user-llm-providers/ModelPicker.store'
import { useUserLlmProvidersStore } from '@/modules/user-llm-providers/UserLlmProviders.store'

// A saved API key changed on another device (the event id is the provider
// id; only masked state is ever exposed). Reload the user's providers +
// masked-key map. `load()` refetches its own scoped, sanitized view.
// (The store also self-gates on this perm; the registry gate is for parity
// with the other handlers.)
registerSync('api_key', {
  onEvent: () => {
    void useUserLlmProvidersStore.getState().load()
  },
  onResync: () => {
    void useUserLlmProvidersStore.getState().load()
  },
  requiredPermission: Permissions.UserLlmProvidersRead,
})

// An admin changed a provider or model. The user's accessible-providers
// view (and the chat model picker) may have changed — each store refetches
// its OWN group-scoped, sanitized view, so the only thing this notification
// discloses is "something changed".
const reloadUserProviders = () => {
  void useUserLlmProvidersStore.getState().load()
  void useModelPickerStore.getState().loadProviders()
}

registerSync('user_llm_provider', {
  onEvent: reloadUserProviders,
  onResync: reloadUserProviders,
  requiredPermission: Permissions.UserLlmProvidersRead,
})
