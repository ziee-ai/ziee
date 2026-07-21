import type { StoreSet } from '@ziee/framework/store-kit'
import type { RuntimeEngine, RuntimeUpdateCheck } from '../../types'

export const runtimeUpdateState = {
  updateChecks: new Map<RuntimeEngine, RuntimeUpdateCheck>() as Map<RuntimeEngine, RuntimeUpdateCheck>,
  checking: new Map<RuntimeEngine, boolean>() as Map<RuntimeEngine, boolean>,
  error: null as string | null,
}

export type RuntimeUpdateState = typeof runtimeUpdateState
export type RuntimeUpdateSet = StoreSet<RuntimeUpdateState>
export type RuntimeUpdateGet = () => RuntimeUpdateState
