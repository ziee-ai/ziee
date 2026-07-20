import type {
  ProviderCatalogEntry,
  WebSearchSettings,
} from '@/api-client/types'

/** The eager state shape — shared by the store + its lazy action files (so an
 *  action's `set` mutator is typed without importing the store). */
export interface WebSearchAdminState {
  settings: WebSearchSettings | null
  providers: ProviderCatalogEntry[]
  loading: boolean
  /** Global settings (enable / chain / caps) save in flight. */
  savingSettings: boolean
  /** Provider key being saved (its registry key), or null — scopes the spinner. */
  savingProvider: string | null
  error: string | null
}

export const webSearchAdminInitialState: WebSearchAdminState = {
  settings: null,
  providers: [],
  loading: false,
  savingSettings: false,
  savingProvider: null,
  error: null,
}
