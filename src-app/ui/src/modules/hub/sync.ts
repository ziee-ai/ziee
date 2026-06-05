import { registerSync } from '@/core/sync'
import { reloadAllTabs } from '@/modules/hub/stores/hub-catalog-store'

// The hub catalog version was pinned/refreshed (singleton; event id is nil).
// Reload every hub category tab so stale per-category lists pick up the new
// catalog.
registerSync('hub_settings', {
  onEvent: () => {
    void reloadAllTabs()
  },
  onResync: () => {
    void reloadAllTabs()
  },
})
