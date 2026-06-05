import { Permissions } from '@/api-client/types'
import { registerSync } from '@/core/sync'
import { useSandboxResourceLimitsStore } from '@/modules/code-sandbox/stores/SandboxResourceLimits.store'

// Code-sandbox resource-limit settings (singleton). Refetch on a remote
// change (the event id is nil — it's a singleton row).
const reload = () => {
  void useSandboxResourceLimitsStore.getState().loadLimits()
}

registerSync('code_sandbox_settings', {
  onEvent: reload,
  onResync: reload,
  requiredPermission: Permissions.CodeSandboxResourceLimitsRead,
})
