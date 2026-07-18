/**
 * TunnelAuth store — desktop-only state for the phone-facing auth surface served
 * over the ngrok tunnel. Keeps the magic-link exchange + phone password login
 * behind a store boundary (single error slot, StrictMode double-exchange guard,
 * one Stores.Auth integration point). Not a routable `Stores.X` store — consumed
 * via the exported hook.
 */

import { ApiClient } from '@/api-client'
import type { AuthConfigResponse, AuthResponse } from '@/api-client/types'
import { defineStore } from '@ziee/framework/store-kit'
import { Stores } from '@ziee/framework/stores'

function applySession(res: AuthResponse): void {
  // Hand the whole pair to the shared Auth store: it captures the body refresh
  // token, records expires_in, and schedules the proactive silent refresh. On
  // refresh failure the shared store clears auth and AuthGuard bounces back to
  // PhoneAuthPage (correct for a remote phone session).
  Stores.Auth.setAuthFromAutoLogin({
    user: res.user,
    access_token: res.access_token,
    refresh_token: res.refresh_token,
    expires_in: res.expires_in,
  })
}

export const TunnelAuth = defineStore('TunnelAuth', {
  state: {
    authConfig: null as AuthConfigResponse | null,
    loadingConfig: false,
    configError: null as string | null,
    submittingPassword: false,
    passwordError: null as string | null,
    exchangingToken: null as string | null,
    exchangeError: null as string | null,
  },
  actions: (set, get) => ({
    loadAuthConfig: async () => {
      if (get().loadingConfig) return
      set({ loadingConfig: true, configError: null })
      try {
        const cfg = await ApiClient.Auth.getConfig(undefined, undefined)
        set({ authConfig: cfg, loadingConfig: false })
      } catch (e) {
        set({
          loadingConfig: false,
          configError: e instanceof Error ? e.message : 'Failed to load login config',
        })
      }
    },
    phonePasswordLogin: async (password: string) => {
      if (get().submittingPassword) return
      set({ submittingPassword: true, passwordError: null })
      try {
        const res = await ApiClient.Auth.loginPasswordOnly({ password }, undefined)
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
      // Dedupe StrictMode double-mount + browser-refresh: no-op if already
      // exchanging THIS token.
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
  }),
})

export const useTunnelAuthStore = TunnelAuth.store
