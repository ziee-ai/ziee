import type { StoreSet } from '@ziee/framework/store-kit'
import type { ProviderWithModels } from '@/api-client/types'

export const apiKeysStepState = {
  providers: [] as ProviderWithModels[],
  userKeys: {} as Record<string, { masked_key: string }>,
  enteredApiKeys: {} as Record<string, string>,
  loadingProviders: false,
  providersError: null as string | null,
}

export type ApiKeysStepState = typeof apiKeysStepState
export type ApiKeysStepSet = StoreSet<ApiKeysStepState>
export type ApiKeysStepGet = () => ApiKeysStepState
