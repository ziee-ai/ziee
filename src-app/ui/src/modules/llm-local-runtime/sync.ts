import { registerSync } from '@/core/sync'
import { useRuntimeVersionStore } from '@/modules/llm-local-runtime/stores/RuntimeVersion.store'

// Admin local-runtime engine versions. Reload the version list on a remote
// download/delete/default change.
const reload = () => {
  void useRuntimeVersionStore.getState().loadVersions()
}

registerSync('runtime_version', {
  onEvent: reload,
  onResync: reload,
})
