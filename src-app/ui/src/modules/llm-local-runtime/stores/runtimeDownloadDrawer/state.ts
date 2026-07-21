import type { RuntimeEngine } from '../../types'
import type { StoreSet } from '@ziee/framework/store-kit'

export const runtimeDownloadDrawerState = {
  open: false,
  engine: null as RuntimeEngine | null,
}

export type RuntimeDownloadDrawerState = typeof runtimeDownloadDrawerState
export type RuntimeDownloadDrawerSet = StoreSet<RuntimeDownloadDrawerState>
export type RuntimeDownloadDrawerGet = () => RuntimeDownloadDrawerState
