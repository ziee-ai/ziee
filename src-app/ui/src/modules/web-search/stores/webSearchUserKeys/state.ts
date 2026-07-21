import type { StoreSet } from '@ziee/framework/store-kit'
import type { UserProviderKeyCatalogEntry } from '@/api-client/types'

export const webSearchUserKeysState = {
  providers: [] as UserProviderKeyCatalogEntry[],
  loading: false,
  savingProvider: null as string | null,
  error: null as string | null,
}

export type WebSearchUserKeysState = typeof webSearchUserKeysState
export type WebSearchUserKeysSet = StoreSet<WebSearchUserKeysState>
export type WebSearchUserKeysGet = () => WebSearchUserKeysState
