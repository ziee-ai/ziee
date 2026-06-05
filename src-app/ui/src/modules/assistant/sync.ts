import { registerSync } from '@/core/sync'
import { useUserAssistantsStore } from '@/modules/assistant/stores/UserAssistants.store'

// `loadUserAssistants` early-returns once `isInitialized` is set, so a
// plain re-call would no-op. Reset the flag first to force a refetch on a
// remote change. (Small per-user list → full reload is fine.)
const reloadUserAssistants = () => {
  useUserAssistantsStore.setState({ isInitialized: false })
  void useUserAssistantsStore.getState().loadUserAssistants()
}

registerSync('assistant', {
  onEvent: reloadUserAssistants,
  onResync: reloadUserAssistants,
})
