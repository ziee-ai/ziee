import type { StoreSet } from '@ziee/framework/store-kit'

export const appState = {
  needsSetup: null as boolean | null,
  isCheckingSetup: false,
  isSettingUpAdmin: false,
  setupError: null as string | null,
}

export type AppState = typeof appState
export type AppSet = StoreSet<AppState>
export type AppGet = () => AppState
