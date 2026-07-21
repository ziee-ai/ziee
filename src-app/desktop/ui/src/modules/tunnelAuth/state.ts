import type { AuthConfigResponse } from '@/api-client/types'
import type { StoreSet } from '@ziee/framework/store-kit'

export const tunnelAuthState = {
  authConfig: null as AuthConfigResponse | null,
  loadingConfig: false,
  configError: null as string | null,
  submittingPassword: false,
  passwordError: null as string | null,
  exchangingToken: null as string | null,
  exchangeError: null as string | null,
}

export type TunnelAuthState = typeof tunnelAuthState
export type TunnelAuthSet = StoreSet<TunnelAuthState>
export type TunnelAuthGet = () => TunnelAuthState
