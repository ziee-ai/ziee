import { create } from 'zustand'
import { persist, subscribeWithSelector } from 'zustand/middleware'
import type { StoreProxy } from '@/core/stores'

export type ThemePreference = 'light' | 'dark' | 'system'

interface ConfigClientState {
  themePreference: ThemePreference
}

// Augment RegisteredStores for IntelliSense
declare module '../../core/stores' {
  interface RegisteredStores {
    ConfigClient: StoreProxy<ConfigClientState>
  }
}

const defaultState: ConfigClientState = {
  themePreference: 'system',
}

export const useConfigClientStore = create<ConfigClientState>()(
  subscribeWithSelector(
    persist((): ConfigClientState => defaultState, {
      name: 'config-client-storage',
      partialize: state => ({ themePreference: state.themePreference }),
    }),
  ),
)

// Config actions
export const setThemePreference = (preference: ThemePreference): void => {
  useConfigClientStore.setState({ themePreference: preference })
}

export const getThemePreference = (): ThemePreference => {
  return useConfigClientStore.getState().themePreference
}
