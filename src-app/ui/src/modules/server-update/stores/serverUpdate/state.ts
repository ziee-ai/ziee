import type { StoreSet } from '@ziee/framework/store-kit'

export const serverUpdateState = {
  currentVersion: null as string | null,
  latestVersion: null as string | null,
  updateAvailable: false,
  releaseUrl: null as string | null,
  notes: null as string | null,
  enabled: true,
  checkedAt: null as string | null,
  dismissed: false,
  loading: false,
  error: null as string | null,
}

export type ServerUpdateState = typeof serverUpdateState
export type ServerUpdateSet = StoreSet<ServerUpdateState>
export type ServerUpdateGet = () => ServerUpdateState
