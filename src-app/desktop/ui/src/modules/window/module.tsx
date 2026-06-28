/**
 * Window Management Module
 *
 * Desktop-specific module for window controls
 */

import { createModule, type AppModule } from '@ziee/ui-core'
import { useWindowStore } from '@ziee/desktop/modules/window/store'

// Desktop modules use the same AppModule interface as core modules
const windowModule: AppModule = createModule({
  metadata: {
    name: 'window',
    version: '1.0.0',
    description: 'Window management for desktop',
  },

  // Routes (if any UI is needed)
  routes: [
    // Example: Settings page for window preferences
    // {
    //   path: '/settings/window',
    //   component: lazy(() => import('./pages/WindowSettings')),
    //   requiresAuth: true,
    // }
  ],

  // Stores
  stores: [
    {
      name: 'Window',
      store: useWindowStore,
    },
  ],

  // Module initialization
  initialize: () => {
    console.log('Window module initialized')

    // Sync the store with the ACTUAL OS window state on startup so
    // `isMaximized` doesn't sit at the stale `false` default until the
    // user first toggles it (a window restored maximized from a previous
    // session would otherwise render the wrong titlebar control). Fire-
    // and-forget; the action swallows its own errors. Guarded on Tauri so
    // the phone/web build (no native window) is a no-op.
    if (window.__TAURI__) {
      void useWindowStore.getState().checkIsMaximized()
    }
  },
})

export default windowModule
