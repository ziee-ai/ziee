import { create } from 'zustand'
import { persist, subscribeWithSelector } from 'zustand/middleware'
import type { StoreProxy } from '@/core/stores'

export type ThemePreference = 'light' | 'dark' | 'system'

interface ConfigClientState {
  themePreference: ThemePreference

  // Actions
  setThemePreference: (preference: ThemePreference) => void
  getThemePreference: () => ThemePreference
}

// Augment RegisteredStores for IntelliSense
declare module '../../core/stores' {
  interface RegisteredStores {
    ConfigClient: StoreProxy<ConfigClientState>
  }
}

const defaultState = {
  themePreference: 'system' as ThemePreference,
}

export const useConfigClientStore = create<ConfigClientState>()(
  subscribeWithSelector(
    persist(
      (set, get): ConfigClientState => ({
        ...defaultState,

        // Actions
        setThemePreference: (preference: ThemePreference) => {
          set({ themePreference: preference })
        },

        getThemePreference: () => {
          return get().themePreference
        },
      }),
      {
        name: 'config-client-storage',
        partialize: state => ({ themePreference: state.themePreference }),
      },
    ),
  ),
)
