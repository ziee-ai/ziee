import type { StoreSet } from '@ziee/framework/store-kit'
import type { RuntimeVersionResponse } from '@/api-client/types'

export const runtimeVersionState = {
  versions: [] as RuntimeVersionResponse[],
  isInitialized: false,
  loading: false,
  downloading: new Map<string, boolean>(), // version_id -> downloading
  settingDefault: new Map<string, boolean>(),
  deleting: new Map<string, boolean>(),
  error: null as string | null,
}

export type RuntimeVersionState = typeof runtimeVersionState
export type RuntimeVersionSet = StoreSet<RuntimeVersionState>
export type RuntimeVersionGet = () => RuntimeVersionState
