import type { StoreSet } from '@ziee/framework/store-kit'
import type { UserConnectorKeyCatalogEntry } from '@/api-client/types'

export const litSearchUserKeysState = {
  connectors: [] as UserConnectorKeyCatalogEntry[],
  loading: false,
  savingConnector: null as string | null,
  error: null as string | null,
}

export type LitSearchUserKeysState = typeof litSearchUserKeysState
export type LitSearchUserKeysSet = StoreSet<LitSearchUserKeysState>
export type LitSearchUserKeysGet = () => LitSearchUserKeysState
