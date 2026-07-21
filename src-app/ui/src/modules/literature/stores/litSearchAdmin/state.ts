import type { ConnectorCatalogEntry, LitSearchSettings } from '@/api-client/types'
import type { StoreSet } from '@ziee/framework/store-kit'

export const litSearchAdminState = {
  settings: null as LitSearchSettings | null,
  connectors: [] as ConnectorCatalogEntry[],
  loading: false,
  savingSettings: false,
  /** Connector key being saved, or null (scopes the spinner to one form). */
  savingConnector: null as string | null,
  error: null as string | null,
}

export type LitSearchAdminState = typeof litSearchAdminState
export type LitSearchAdminSet = StoreSet<LitSearchAdminState>
export type LitSearchAdminGet = () => LitSearchAdminState
