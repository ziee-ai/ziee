import type { StoreSet } from '@ziee/framework/store-kit'
import type { FileVersion } from '@/api-client/types'

export const fileVersionsState = {
  /** Version lists, keyed by file ID. */
  versionsByFile: new Map<string, FileVersion[]>(),
  /** File IDs currently loading versions. */
  versionsLoadingSet: new Set<string>(),
  /** Cached text of a specific version, keyed `${fileId}:${version}`. */
  versionTextCache: new Map<string, string>(),
  /** Keys currently loading version text. */
  versionTextLoadingSet: new Set<string>(),
}

export type FileVersionsState = typeof fileVersionsState
export type FileVersionsSet = StoreSet<FileVersionsState>
export type FileVersionsGet = () => FileVersionsState
