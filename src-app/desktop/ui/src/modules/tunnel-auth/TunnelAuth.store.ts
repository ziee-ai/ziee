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

// Module-local shadow of the current phone session's refresh token +
// the proactive-refresh timer handle. The server-UI `Stores.Auth`
// holds the refresh token internally but doesn't expose it on the
// public state shape; rather than reach into its private fields, the
// magic-link exchange + password login responses give us the refresh
// token directly via `AuthResponse.refresh_token` and we shadow it
// here. Single phone session per bundle instance.
let refreshToken: string | null = null
let refreshTimer: ReturnType<typeof setTimeout> | null = null

function scheduleRefresh(expiresInSeconds: number | undefined): void {
  if (refreshTimer) {
    clearTimeout(refreshTimer)
    refreshTimer = null
  }
  if (!expiresInSeconds || expiresInSeconds <= 0) return
  // Refresh at 80% of token lifetime (matches the desktop auto-login
  // path's behavior). For a 1-hour access token, that's ~48 min — well
  // before expiry, with enough slack that a slow network won't strand
  // the phone session if the rotation itself is slow.
  const refreshInMs = Math.floor(expiresInSeconds * 0.8 * 1000)
  refreshTimer = setTimeout(() => {
    void refreshPhoneSession()
  }, refreshInMs)
}

async function refreshPhoneSession(): Promise<void> {
  if (!refreshToken) return
  try {
    // Call the standard `/api/auth/refresh` with the cached refresh
    // token. The server rotates BOTH tokens (refresh token is
    // single-use post-H3) and returns a `TokenPair` (no `user` —
    // the user obviously didn't change during a refresh, so we
    // preserve whatever's in `Stores.Auth.user`).
    const pair = await ApiClient.Auth.refresh(
      { refresh_token: refreshToken },
      undefined,
    )
    refreshToken = pair.refresh_token
    Stores.Auth.setAuthFromAutoLogin({
      // null tells setAuthFromAutoLogin to keep the existing user
      // (it re-fetches /me to revalidate the row internally).
      user: null,
      access_token: pair.access_token,
      refresh_token: pair.refresh_token,
      expires_in: pair.expires_in,
    })
    scheduleRefresh(pair.expires_in)
  } catch {
    // Refresh failed (token revoked, server gone, network out).
    // Drop the shadow + let the next API call bounce the user back
    // to PhoneAuthPage via AuthGuard.
    refreshToken = null
    if (refreshTimer) {
      clearTimeout(refreshTimer)
      refreshTimer = null
    }
  }
}

function applySession(res: AuthResponse): void {
  // Shadow the refresh token for the next proactive rotation.
  refreshToken = res.refresh_token
  Stores.Auth.setAuthFromAutoLogin({
    user: res.user,
    access_token: res.access_token,
    refresh_token: res.refresh_token,
    expires_in: res.expires_in,
  })
  // Schedule proactive refresh. Without this, the phone's 1-hour
  // access token silently dies and AuthGuard bounces the user back
  // to PhoneAuthPage mid-session.
  scheduleRefresh(res.expires_in)
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
