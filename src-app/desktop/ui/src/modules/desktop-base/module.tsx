/**
 * Desktop Base Module
 *
 * Provides core desktop functionality:
 * - API client override (getBaseURL replacement)
 * - Desktop environment detection
 * - Tauri initialization
 * - Desktop auto-login
 */

import { createModule, type AppModule } from '@ziee/ui-core'
import { Stores } from '@/core/stores'
import type { AutoLoginResponse } from '@/modules/auth/Auth.store'

const desktopBaseModule: AppModule = createModule({
  metadata: {
    name: 'desktop-base',
    version: '1.0.0',
    description: 'Core desktop functionality and API client override',
  },

  routes: [],
  stores: [],

  initialize: async () => {
    console.log('[Desktop] Desktop base module initialized')

    // Check if Tauri is available
    if (!window.__TAURI__) {
      console.warn('[Desktop] Tauri not available - running in web mode')
      return
    }

    console.log('[Desktop] Tauri environment detected')

    // Get API base URL
    let baseUrl: string
    try {
      const { getBaseUrl } = await import('./getBaseURL')
      baseUrl = await getBaseUrl()
      console.log('[Desktop] API base URL configured:', baseUrl)
    } catch (error) {
      console.error('[Desktop] Failed to configure API base URL:', error)
      return
    }

    // Check if already authenticated
    const currentToken = Stores.Auth.token
    if (currentToken) {
      console.log('[Desktop] Already authenticated, skipping auto-login')
      return
    }

    // Perform desktop auto-login
    try {
      console.log('[Desktop] Performing auto-login...')
      const response = await fetch(`${baseUrl}/api/desktop/auto-login`, {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
        },
      })

      if (!response.ok) {
        throw new Error(`Auto-login failed: ${response.status} ${response.statusText}`)
      }

      const authData: AutoLoginResponse = await response.json()
      console.log('[Desktop] Auto-login successful for user:', authData.user.username)

      // Set auth state
      Stores.Auth.setAuthFromAutoLogin(authData)
    } catch (error) {
      console.error('[Desktop] Auto-login failed:', error)
    }
  },

  cleanup: async () => {
    console.log('[Desktop] Desktop base module cleaned up')
  },
})

export default desktopBaseModule
