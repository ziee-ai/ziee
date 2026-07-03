/**
 * Desktop Base Module
 *
 * Provides core desktop functionality:
 * - Desktop environment detection
 * - Tauri-driven auto-login (retried with backoff until the embedded
 *   server is ready)
 * - PERMANENT sessions: the shared Auth store's silent refresh (75% of
 *   lifetime + sleep/wake watchdog) keeps the token fresh, and a
 *   registered fallback re-mints via `auto_login` if a refresh ever
 *   fails — the local desktop user is never bounced to a login page.
 */

import { createModule, type AppModule } from '@ziee/ui-core'
import { Stores, type StoreProxy } from '@/core/stores'
import { invoke } from '@tauri-apps/api/core'
import type { AutoLoginResponse } from '@/modules/auth/Auth.store'
import { useBootstrapStore } from '@ziee/desktop/modules/desktop-base/Bootstrap.store'

declare module '@/core/stores' {
  interface RegisteredStores {
    Bootstrap: StoreProxy<ReturnType<typeof useBootstrapStore.getState>>
  }
}

// Retry config — exponential with cap, hard deadline.
const RETRY_BACKOFF_MS = [500, 1000, 2000, 4000, 5000]
const RETRY_DEADLINE_MS = 30_000

let cleanupRequested = false

function backoff(attempt: number): number {
  return RETRY_BACKOFF_MS[Math.min(attempt, RETRY_BACKOFF_MS.length - 1)]
}

function applyTokens(authData: AutoLoginResponse): void {
  // The shared Auth store captures the body refresh token + expires_in
  // and runs the proactive silent refresh + sleep/wake watchdog. (This
  // module previously kept its own single 80%-of-lifetime setTimeout —
  // which never fires when the machine sleeps through it, the root
  // cause of the desktop mid-work auto-logout — deleted in favor of the
  // shared machinery.)
  Stores.Auth.setAuthFromAutoLogin(authData)
}

async function runAutoLoginWithRetry(): Promise<void> {
  const bootstrap = Stores.Bootstrap.__state
  const startedAt = Date.now()
  let attempt = 0

  bootstrap.setStatus('retrying', 'Starting up…')
  bootstrap.setAttempt(0)

  while (!cleanupRequested) {
    try {
      const authData = await invoke<AutoLoginResponse>('auto_login')
      // The Tauri command always returns a non-null user (the OAuth
      // null-user path is web-only). Narrow before touching .username.
      if (!authData.user) {
        throw new Error('auto_login returned null user (unexpected)')
      }
      console.log(
        '[Desktop] Auto-login successful for user:',
        authData.user.username,
        attempt > 0 ? `(after ${attempt} retries)` : '',
      )
      applyTokens(authData)
      Stores.Bootstrap.__state.setStatus('succeeded')
      return
    } catch (error) {
      attempt += 1
      const elapsed = Date.now() - startedAt
      const remaining = RETRY_DEADLINE_MS - elapsed
      const msg = error instanceof Error ? error.message : String(error)

      if (remaining <= 0) {
        console.error(
          '[Desktop] Auto-login exceeded wall-clock deadline after',
          attempt,
          'attempts. Last error:',
          msg,
        )
        Stores.Bootstrap.__state.setStatus(
          'failed',
          'Backend failed to start. Try restarting Ziee.',
        )
        return
      }

      const wait = Math.min(backoff(attempt - 1), remaining)
      console.warn(
        `[Desktop] Auto-login attempt ${attempt} failed (${msg}); retrying in ${wait}ms`,
      )
      Stores.Bootstrap.__state.setAttempt(attempt)
      await new Promise(resolve => setTimeout(resolve, wait))
    }
  }
}

const desktopBaseModule: AppModule = createModule({
  metadata: {
    name: 'desktop-base',
    version: '1.0.0',
    description: 'Core desktop functionality and auto-login bootstrap',
  },

  routes: [],
  stores: [
    {
      name: 'Bootstrap',
      store: useBootstrapStore,
    },
  ],

  initialize: async () => {
    console.log('[Desktop] Desktop base module initialized')

    // Note: `Stores.AppMode.setMultiUserMode(false)` is set
    // synchronously in `desktop/ui/src/main.tsx` BEFORE the React
    // render, so multi-user-only widgets never render even briefly.
    // Don't move it here — async initialize lets a render frame
    // sneak in with multiUserMode=true and flash the wrong UI.

    if (!window.__TAURI__) {
      console.warn('[Desktop] Tauri not available - running in web mode')
      return
    }

    cleanupRequested = false
    console.log('[Desktop] Tauri environment detected; starting auto-login')

    // Permanence hook: if the shared silent refresh ever fails with a
    // 401 (revoked/expired refresh token — e.g. the machine slept past
    // the full session length), re-mint locally via auto_login instead
    // of clearing auth. The embedded server trusts the local user, so
    // the desktop session never ends up on a login screen.
    Stores.Auth.setRefreshFallback(async () => {
      console.log('[Desktop] Token refresh failed; re-minting via auto_login')
      await runAutoLoginWithRetry()
    })

    void runAutoLoginWithRetry()
  },

  cleanup: async () => {
    cleanupRequested = true
    Stores.Auth.setRefreshFallback(null)
    Stores.Bootstrap.__state.reset()
    console.log('[Desktop] Desktop base module cleaned up')
  },
})

export default desktopBaseModule
