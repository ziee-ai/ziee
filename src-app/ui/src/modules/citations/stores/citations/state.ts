import type { StoreSet } from '@ziee/framework/store-kit'
import type { BibliographyEntry } from '@/api-client/types'

export const citationsState = {
  entries: [] as BibliographyEntry[],
  loading: false,
  importing: false,
  verifying: false,
  error: null as string | null,
  /** When set, the store scopes to a project's reference list. */
  projectId: null as string | null,
}

export type CitationsState = typeof citationsState
export type CitationsSet = StoreSet<CitationsState>
export type CitationsGet = () => CitationsState
