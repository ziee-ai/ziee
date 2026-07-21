import type { StoreSet } from '@ziee/framework/store-kit'

export const userProviderKeysState = {
  keys: {} as Record<string, { masked_key: string }>,
  saving: false,
  // Renamed from `__init__` (a reserved store-kit lifecycle key).
  initialized: false,
}

export type UserProviderKeysState = typeof userProviderKeysState
export type UserProviderKeysSet = StoreSet<UserProviderKeysState>
export type UserProviderKeysGet = () => UserProviderKeysState
