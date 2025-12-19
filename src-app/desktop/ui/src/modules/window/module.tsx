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

    // Desktop-specific initialization
    // e.g., set up Tauri event listeners
    if (window.__TAURI__) {
      // Listen for window events
      // window.__TAURI__.event.listen('window-resized', ...)
    }
  },
})

export default windowModule
