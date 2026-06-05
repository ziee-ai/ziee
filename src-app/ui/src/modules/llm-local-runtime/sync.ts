import { Permissions } from '@/api-client/types'
import { registerSync } from '@/core/sync'
import { useRuntimeConfigStore } from '@/modules/llm-local-runtime/stores/RuntimeConfig.store'
import { useRuntimeVersionStore } from '@/modules/llm-local-runtime/stores/RuntimeVersion.store'

// Admin local-runtime engine versions. Reload the version list on a remote
// download/delete/default change.
const reload = () => {
  void useRuntimeVersionStore.getState().loadVersions()
}

registerSync('runtime_version', {
  onEvent: reload,
  onResync: reload,
  requiredPermission: Permissions.RuntimeVersionRead,
})

// Deployment-wide runtime engine settings (singleton; event id is nil).
const reloadSettings = () => {
  void useRuntimeConfigStore.getState().loadSettings()
}

registerSync('runtime_settings', {
  onEvent: reloadSettings,
  onResync: reloadSettings,
  requiredPermission: Permissions.RuntimeSettingsRead,
})
