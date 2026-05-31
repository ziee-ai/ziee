// Main hub module types — registers the catalog + updates stores so
// callers can do `Stores.HubCatalog.catalog` / `Stores.HubUpdates.updates`
// with full type safety.
import type { StoreProxy } from '@/core/stores'
import type { useHubCatalogStore } from '@/modules/hub/stores/hub-catalog-store'
import type { useHubUpdatesStore } from '@/modules/hub/stores/hub-updates-store'

declare module '@/core/stores' {
  interface RegisteredStores {
    HubCatalog: StoreProxy<ReturnType<typeof useHubCatalogStore.getState>>
    HubUpdates: StoreProxy<ReturnType<typeof useHubUpdatesStore.getState>>
  }
}

export {}
