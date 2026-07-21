import type { StoreSet } from '@ziee/framework/store-kit'

export const addRemoteLlmModelDrawerState = {
  open: false,
  loading: false,
  providerId: null as string | null,
  providerType: null as string | null,
}

export type AddRemoteLlmModelDrawerState = typeof addRemoteLlmModelDrawerState
export type AddRemoteLlmModelDrawerSet = StoreSet<AddRemoteLlmModelDrawerState>
export type AddRemoteLlmModelDrawerGet = () => AddRemoteLlmModelDrawerState
