import type { StoreSet } from '@ziee/framework/store-kit'
import type { InstanceResponse, VersionUsageResponse } from '@/api-client/types'
import type { RuntimeEngine } from '../../types'

export const runtimeModelUsageState = {
  // Per-engine usage snapshot (versions + the models that resolve to each).
  usage: new Map<RuntimeEngine, VersionUsageResponse>(),
  // Per-engine load-in-flight.
  loading: new Map<RuntimeEngine, boolean>(),
  // Per-model action-in-flight (start/stop/restart/swap), keyed by model id.
  acting: new Map<string, boolean>(),
  // Per-model running-instance detail, lazily loaded. `null` = fetched, none.
  instances: new Map<string, InstanceResponse | null>(),
  error: null as string | null,
}

export type RuntimeModelUsageState = typeof runtimeModelUsageState
export type RuntimeModelUsageSet = StoreSet<RuntimeModelUsageState>
export type RuntimeModelUsageGet = () => RuntimeModelUsageState
