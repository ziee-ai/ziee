/**
 * Desktop Base Module
 *
 * Provides core desktop functionality:
 * - API client override (getBaseURL replacement)
 * - Desktop environment detection
 * - Tauri initialization
 */

import { createModule, type AppModule } from '@ziee/ui-core'

const desktopBaseModule: AppModule = createModule({
  metadata: {
    name: 'desktop-base',
    version: '1.0.0',
    description: 'Core desktop functionality and API client override',
  },

  routes: [],
  stores: [],

  sidebar: undefined,

  initialize: async () => {
    console.log('[Desktop] Desktop base module initialized')

    // Check if Tauri is available
    if (window.__TAURI__) {
      console.log('[Desktop] Tauri environment detected')
    } else {
      console.warn('[Desktop] Tauri not available - running in web mode')
    }

    // Test API connection
    try {
      const { getBaseUrl } = await import('./getBaseURL')
      const baseUrl = await getBaseUrl()
      console.log('[Desktop] API base URL configured:', baseUrl)
    } catch (error) {
      console.error('[Desktop] Failed to configure API base URL:', error)
    }
  },

  cleanup: async () => {
    console.log('[Desktop] Desktop base module cleaned up')
  },
})

export default desktopBaseModule
