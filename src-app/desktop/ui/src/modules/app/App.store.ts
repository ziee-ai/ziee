/**
 * DELIBERATE DIVERGENCE from core's App.store.
 *
 * Why: core's `App.store.checkSetupStatus()` hits
 * `/api/app/setup/status` and, if the response says `needs_setup:
 * true`, `app/module.tsx::initialize` does `window.location.href =
 * '/setup'` — bouncing the entire SPA to the setup page.
 *
 * On desktop the admin is bootstrapped by the Tauri host BEFORE the
 * webview is created (see `desktop/tauri/src/modules/auth/bootstrap.rs`),
 * so `needs_setup` is always false in production. But:
 *   - the e2e specs intentionally mock the status API to return
 *     `needs_setup: true` (`desktop-no-setup-redirect.spec.ts`) and
 *     assert that desktop does NOT redirect; AND
 *   - even in production, briefly bouncing to /setup before
 *     auto-login lands would flash the wrong page.
 *
 * This override pins `needsSetup` to `false` at construction time and
 * neuters `checkSetupStatus()` to do nothing. `setupAdmin` stays
 * functional (callable, though there's no UI on desktop that exercises
 * it).
 */

import { create } from 'zustand'
import { ApiClient } from '@/api-client'
import type { SetupAdminRequest } from '@/api-client/types'
import type { StoreProxy } from '@/core/stores'

interface AppState {
  needsSetup: boolean | null
  isCheckingSetup: boolean
  isSettingUpAdmin: boolean
  setupError: string | null

  checkSetupStatus: () => Promise<void>
  setupAdmin: (request: SetupAdminRequest) => Promise<void>
  clearSetupError: () => void
}

declare module '../../core/stores' {
  interface RegisteredStores {
    App: StoreProxy<AppState>
  }
}

export const useAppStore = create<AppState>((set, get) => ({
  // Pin to `false` — desktop's Tauri host always bootstraps the admin
  // before the webview opens. Skipping `null` here also keeps any
  // `needsSetup === null` spinner branches (core AuthGuard had one;
  // the desktop override doesn't) from briefly firing.
  needsSetup: false,
  isCheckingSetup: false,
  isSettingUpAdmin: false,
  setupError: null,

  // Pin needsSetup=false; never hit the API. Core's app/module.tsx
  // calls this on init and then redirects to /setup if the result is
  // true — we don't want that path on desktop.
  checkSetupStatus: async () => {
    // No-op. `needsSetup` stays at `false` from initial state.
  },

  setupAdmin: async (request: SetupAdminRequest) => {
    const state = get()
    if (state.isSettingUpAdmin) return
    set({ isSettingUpAdmin: true, setupError: null })
    try {
      await ApiClient.App.setupAdmin(request, undefined)
      set({ isSettingUpAdmin: false, needsSetup: false, setupError: null })
    } catch (error: unknown) {
      const err = error as { response?: { data?: { message?: string } }; message?: string }
      const message =
        err?.response?.data?.message ||
        err?.message ||
        'Setup failed. Please try again.'
      set({ isSettingUpAdmin: false, setupError: message })
      throw error
    }
  },

  clearSetupError: () => {
    set({ setupError: null })
  },
}))
