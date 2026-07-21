import type { StoreSet } from '@ziee/framework/store-kit'
import type { ProviderWithModels } from '@/api-client/types'

export const userLlmProvidersState = {
  providers: [] as ProviderWithModels[],
  userKeys: {} as Record<string, { masked_key: string }>,
  loading: false,
  saving: false,
  error: null as string | null,
}

export type UserLlmProvidersState = typeof userLlmProvidersState
export type UserLlmProvidersSet = StoreSet<UserLlmProvidersState>
export type UserLlmProvidersGet = () => UserLlmProvidersState
