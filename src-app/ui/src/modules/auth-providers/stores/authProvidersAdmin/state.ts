import type { StoreSet } from '@ziee/framework/store-kit'
import type { AuthProviderResponse } from '@/api-client/types'

export const authProvidersAdminState = {
  providers: [] as AuthProviderResponse[],
  // Start loading so the first paint shows a spinner, not a spurious empty
  // state, before init loads. loadProviders always resets on success/error.
  loading: true,
  saving: false,
  error: null as string | null,
  /// IDs currently mid-test (row Test button spinner). Cleared per-id.
  testingIds: new Set<string>(),
}

export type AuthProvidersAdminState = typeof authProvidersAdminState
export type AuthProvidersAdminSet = StoreSet<AuthProvidersAdminState>
export type AuthProvidersAdminGet = () => AuthProvidersAdminState
