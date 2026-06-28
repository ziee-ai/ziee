import { create } from 'zustand'
import { ApiClient } from '@/api-client'
import type { SetupAdminRequest } from '@/api-client/types'
import type { StoreProxy } from '@/core/stores'

interface AppState {
  needsSetup: boolean | null
  isCheckingSetup: boolean
  isSettingUpAdmin: boolean
  setupError: string | null

  // Actions
  checkSetupStatus: () => Promise<void>
  setupAdmin: (request: SetupAdminRequest) => Promise<void>
  clearSetupError: () => void
}

// Augment the RegisteredStores interface for IntelliSense
declare module '../../core/stores' {
  interface RegisteredStores {
    App: StoreProxy<AppState>
  }
}

export const useAppStore = create<AppState>((set, get) => ({
  needsSetup: null,
  isCheckingSetup: false,
  isSettingUpAdmin: false,
  setupError: null,

  // Actions
  checkSetupStatus: async () => {
    const state = get()
    if (state.isCheckingSetup) {
      return
    }

    set({ isCheckingSetup: true })

    try {
      const response = await ApiClient.App.getSetupStatus(undefined, undefined)
      set({
        needsSetup: response.needs_setup,
        isCheckingSetup: false,
      })
    } catch (error) {
      console.error('Failed to check setup status:', error)
      // If we can't check setup status, assume setup is needed (safe default)
      set({
        needsSetup: true,
        isCheckingSetup: false,
      })
    }
  },

  setupAdmin: async (request: SetupAdminRequest) => {
    const state = get()
    if (state.isSettingUpAdmin) {
      return
    }

    set({ isSettingUpAdmin: true, setupError: null })

    try {
      await ApiClient.App.setupAdmin(request, undefined)
      set({
        isSettingUpAdmin: false,
        needsSetup: false,
        setupError: null,
      })
    } catch (error: any) {
      const message =
        error?.response?.data?.message ||
        error?.message ||
        'Setup failed. Please try again.'
      set({
        isSettingUpAdmin: false,
        setupError: message,
      })
      throw error
    }
  },

  clearSetupError: () => {
    set({ setupError: null })
  },
}))
