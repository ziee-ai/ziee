import { Permissions } from '@/api-client/types'
import { registerSync } from '@/core/sync'
import { reloadAllTabs } from '@/modules/hub/stores/hub-catalog-store'

// The hub catalog version was pinned/refreshed (singleton; event id is nil).
// Reload every hub category tab so stale per-category lists pick up the new
// catalog.
//
// Gate on the perms `reloadAllTabs` actually needs — it calls loadModels +
// loadAssistants + loadServers, none of which self-gate. `hub::models::read`
// is admin-only (migration 37 removed it from the Users group), so a
// non-admin reconnect's `resyncAll` would 403 on `GET /hub/models` without
// this. (NOT `hub::catalog::read` — that's the admin catalog-settings perm,
// unrelated to the three tab reads this refetch performs.)
registerSync('hub_settings', {
  onEvent: () => {
    void reloadAllTabs()
  },
  onResync: () => {
    void reloadAllTabs()
  },
  requiredPermission: {
    allOf: [
      Permissions.HubModelsRead,
      Permissions.HubAssistantsRead,
      Permissions.HubMCPServersRead,
    ],
  },
})
