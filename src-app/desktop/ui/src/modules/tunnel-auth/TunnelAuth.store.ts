/**
 * TunnelAuth store — desktop-only state for the phone-facing auth
 * surface served over the ngrok tunnel.
 *
 * Keeps the magic-link exchange + phone password login behind a
 * store boundary (instead of components calling ApiClient directly)
 * so that:
 *   - Errors flow through one captured `error` slot the UI renders.
 *   - The Strict-Mode double-mount can't fire two exchanges in
 *     parallel (in-flight token is recorded in the store).
 *   - Stores.Auth integration lives in ONE place that we can swap
 *     when the refresh-token rotation lands.
 *
 * NOT registered as a routable store (no `Stores.TunnelAuth` proxy
 * needed) — consumed directly via the exported hook from the two
 * pages in the same module.
 */

import { create } from 'zustand'
import { ApiClient } from '@/api-client'
import { Stores } from '@/core/stores'
import type { AuthConfigResponse, AuthResponse } from '@/api-client/types'

interface TunnelAuthState {
  authConfig: AuthConfigResponse | null
  loadingConfig: boolean
  configError: string | null

  submittingPassword: boolean
  passwordError: string | null

  exchangingToken: string | null
  exchangeError: string | null

  loadAuthConfig: () => Promise<void>
  phonePasswordLogin: (password: string) => Promise<void>
  exchangeMagicLink: (token: string) => Promise<void>
}

function applySession(res: AuthResponse): void {
  // Hand the whole pair to the shared Auth store: it captures the body
  // refresh token into its in-memory shadow, records expires_in, and
  // schedules the proactive silent refresh (75% of lifetime + sleep/wake
  // watchdog). This store used to run its OWN 80% timer alongside — two
  // independent schedulers racing the single-use rotation — deleted when
  // the shared refresh machinery landed. On refresh failure the shared
  // store clears auth and AuthGuard bounces back to PhoneAuthPage,
  // which is correct for a remote phone session.
  Stores.Auth.setAuthFromAutoLogin({
    user: res.user,
    access_token: res.access_token,
    refresh_token: res.refresh_token,
    expires_in: res.expires_in,
  })
}

export const useTunnelAuthStore = create<TunnelAuthState>((set, get) => ({
  authConfig: null,
  loadingConfig: false,
  configError: null,

  submittingPassword: false,
  passwordError: null,

  exchangingToken: null,
  exchangeError: null,

  loadAuthConfig: async () => {
    if (get().loadingConfig) return
    set({ loadingConfig: true, configError: null })
    try {
      const cfg = await ApiClient.Auth.getConfig(undefined, undefined)
      set({ authConfig: cfg, loadingConfig: false })
    } catch (e) {
      set({
        loadingConfig: false,
        configError:
          e instanceof Error ? e.message : 'Failed to load login config',
      })
    }
  },

  phonePasswordLogin: async (password: string) => {
    if (get().submittingPassword) return
    set({ submittingPassword: true, passwordError: null })
    try {
      const res = await ApiClient.Auth.loginPasswordOnly(
        { password },
        undefined,
      )
      applySession(res)
      set({ submittingPassword: false })
    } catch (e) {
      set({
        submittingPassword: false,
        passwordError: e instanceof Error ? e.message : 'Login failed',
      })
      throw e
    }
  },

  exchangeMagicLink: async (token: string) => {
    // Dedupe Strict-Mode double-mount + browser-refresh: if we're
    // already running an exchange for THIS token, no-op.
    if (get().exchangingToken === token) return
    set({ exchangingToken: token, exchangeError: null })
    try {
      const res = await ApiClient.Auth.magicLinkExchange({ token }, undefined)
      applySession(res)
      set({ exchangingToken: null })
    } catch (e) {
      set({
        exchangingToken: null,
        exchangeError: e instanceof Error ? e.message : 'Exchange failed',
      })
      throw e
    }
  },
}))
