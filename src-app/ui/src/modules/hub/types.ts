// Main hub module types — registers the catalog + installed stores so
// callers can do `Stores.HubCatalog.catalog` / `Stores.HubInstalled.items`
// with full type safety.
import type { StoreProxy } from '@/core/stores'
import type { useHubCatalogStore } from '@/modules/hub/stores/hub-catalog-store'
import type { useHubInstalledStore } from '@/modules/hub/stores/hub-installed-store'

declare module '@/core/stores' {
  interface RegisteredStores {
    HubCatalog: StoreProxy<ReturnType<typeof useHubCatalogStore.getState>>
    HubInstalled: StoreProxy<ReturnType<typeof useHubInstalledStore.getState>>
  }
}

export {}
