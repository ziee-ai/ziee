import type { StoreSet } from '@ziee/framework/store-kit'

export const appModeState = {
  multiUserMode: true,
}

export type AppModeState = typeof appModeState
export type AppModeSet = StoreSet<AppModeState>
export type AppModeGet = () => AppModeState
