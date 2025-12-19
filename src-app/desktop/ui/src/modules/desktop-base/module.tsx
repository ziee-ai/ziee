/**
 * Desktop Base Module
 *
 * Provides core desktop functionality:
 * - API client override (getBaseURL replacement)
 * - Desktop environment detection
 * - Tauri initialization
 * - Desktop auto-login (via Tauri command for security)
 * - Proactive token refresh before expiry
 */

import { createModule, type AppModule } from '@ziee/ui-core'
import { Stores } from '@/core/stores'
import { invoke } from '@tauri-apps/api/core'
import type { AutoLoginResponse } from '@/modules/auth/Auth.store'

// Token refresh timer
let refreshTimer: ReturnType<typeof setTimeout> | null = null

/**
 * Perform auto-login and schedule token refresh
 */
async function performAutoLogin(): Promise<void> {
  const authData = await invoke<AutoLoginResponse>('auto_login')
  console.log(
    '[Desktop] Auto-login successful for user:',
    authData.user.username,
  )

  // Set auth state
  Stores.Auth.setAuthFromAutoLogin(authData)

  // Schedule proactive token refresh at 80% of token lifetime
  if (authData.expires_in) {
    const refreshIn = authData.expires_in * 0.8 * 1000 // Convert to ms
    const refreshMinutes = Math.round(refreshIn / 1000 / 60)
    console.log(`[Desktop] Token refresh scheduled in ${refreshMinutes} minutes`)

    // Clear any existing timer
    if (refreshTimer) {
      clearTimeout(refreshTimer)
    }

    // Schedule refresh
    refreshTimer = setTimeout(async () => {
      console.log('[Desktop] Proactively refreshing token...')
      try {
        await performAutoLogin()
      } catch (error) {
        console.error('[Desktop] Token refresh failed:', error)
      }
    }, refreshIn)
  }
}

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

    // Perform desktop auto-login via Tauri command (not REST API for security)
    try {
      console.log('[Desktop] Performing auto-login...')
      await performAutoLogin()
    } catch (error) {
      console.error('[Desktop] Auto-login failed:', error)
    }
  },

  cleanup: async () => {
    // Clear token refresh timer
    if (refreshTimer) {
      clearTimeout(refreshTimer)
      refreshTimer = null
      console.log('[Desktop] Token refresh timer cleared')
    }
    console.log('[Desktop] Desktop base module cleaned up')
  },
})

export default desktopBaseModule
