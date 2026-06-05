import { Permissions } from '@/api-client/types'
import { registerSync } from '@/core/sync'
import { useMemoriesStore } from '@/modules/memory/stores/Memories.store'
import { useMemoryAdminStore } from '@/modules/memory/stores/MemoryAdmin.store'
import { useMemorySettingsStore } from '@/modules/memory/stores/MemorySettings.store'

// Memories is a paginated list; `load()` reloads the current page, which
// surfaces remote creates/edits/deletes that fall on it (a bulk-clear
// arrives as a Delete with a nil id and reloads the page to empty).
registerSync('memory', {
  onEvent: () => {
    void useMemoriesStore.getState().load()
  },
  onResync: () => {
    void useMemoriesStore.getState().load()
  },
})

// Memory settings is a per-user singleton — refetch it.
registerSync('memory_settings', {
  onEvent: () => {
    void useMemorySettingsStore.getState().load()
  },
  onResync: () => {
    void useMemorySettingsStore.getState().load()
  },
})

// Deployment-wide memory admin settings (singleton; event id is nil).
registerSync('memory_admin_settings', {
  onEvent: () => {
    void useMemoryAdminStore.getState().load()
  },
  onResync: () => {
    void useMemoryAdminStore.getState().load()
  },
  requiredPermission: Permissions.MemoryAdminRead,
})
