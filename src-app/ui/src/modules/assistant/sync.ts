import { Permissions } from '@/api-client/types'
import { registerSync } from '@/core/sync'
import { useTemplateAssistantsStore } from '@/modules/assistant/stores/TemplateAssistants.store'
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

// Shared assistant templates. `loadTemplateAssistants` only skips while a
// load is already in flight, so a plain reload refetches the current page.
const reloadTemplates = () => {
  void useTemplateAssistantsStore.getState().loadTemplateAssistants()
}

registerSync('assistant_template', {
  onEvent: reloadTemplates,
  onResync: reloadTemplates,
  requiredPermission: Permissions.AssistantsTemplateRead,
})
