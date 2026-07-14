import type { StoreProxy } from '@ziee/framework/stores'
import type { useAppLayoutStore } from '@/modules/layouts/app-layout/AppLayout.store'

// Shim → @ziee/shell. The generic sidebar slot item types + the `Slots`
// augmentation (sidebarNavigation/Tools/PrimaryActions/Content/Bottom/Footer +
// appBanners) now live in `@ziee/shell/layouts/appLayoutSlots`. Re-exporting
// from it here pulls that module into the app's compilation, so the `declare
// module '@ziee/framework/module-system/types'` slot augmentation stays active
// for every app-side importer of this path (unchanged behavior).
export type {
  SidebarNavItem,
  SidebarToolItem,
  SidebarActionItem,
  SidebarWidgetItem,
} from '@ziee/shell/layouts/appLayoutSlots'
import '@ziee/shell/layouts/appLayoutSlots'

// App-side store-type augmentation stays here — it references the app's
// concrete `useAppLayoutStore` (the store is app-registered; the shell reads it
// only through a typed seam view).
declare module '@ziee/framework/stores' {
  interface RegisteredStores {
    AppLayout: StoreProxy<ReturnType<typeof useAppLayoutStore.getState>>
  }
}
