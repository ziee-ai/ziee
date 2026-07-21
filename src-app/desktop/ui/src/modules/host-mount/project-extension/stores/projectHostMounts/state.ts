import type { StoreSet } from '@ziee/framework/store-kit'
import type { MountEntry } from '@/api-client/types'

export const projectHostMountsState = {
  currentProjectId: null as string | null,
  mounts: [] as MountEntry[],
  loading: false,
  saving: false,
  error: null as string | null,
}

export type ProjectHostMountsState = typeof projectHostMountsState
export type ProjectHostMountsSet = StoreSet<ProjectHostMountsState>
export type ProjectHostMountsGet = () => ProjectHostMountsState
