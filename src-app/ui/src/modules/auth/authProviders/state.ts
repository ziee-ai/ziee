import type { StoreSet } from '@ziee/framework/store-kit'
import type { PublicProvider } from '@/api-client/types'

export const authProvidersState = {
  providers: [] as PublicProvider[],
  isLoading: false,
  error: null as string | null,
  hasLoaded: false,
}

export type AuthProvidersState = typeof authProvidersState
export type AuthProvidersSet = StoreSet<AuthProvidersState>
export type AuthProvidersGet = () => AuthProvidersState
