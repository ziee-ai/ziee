import type { StoreSet } from '@ziee/framework/store-kit'
import type { HubModel } from '@/api-client/types'

export const modelDetailsDrawerState = {
  isOpen: false,
  selectedModel: null as HubModel | null,
  /** True while the fresh manifest is being fetched on open. */
  loading: false,
}

export type ModelDetailsDrawerState = typeof modelDetailsDrawerState
export type ModelDetailsDrawerSet = StoreSet<ModelDetailsDrawerState>
export type ModelDetailsDrawerGet = () => ModelDetailsDrawerState
