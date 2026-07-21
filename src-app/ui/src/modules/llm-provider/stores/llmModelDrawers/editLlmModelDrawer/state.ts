import type { StoreSet } from '@ziee/framework/store-kit'

export const editLlmModelDrawerState = {
  open: false,
  loading: false,
  modelId: null as string | null,
}

export type EditLlmModelDrawerState = typeof editLlmModelDrawerState
export type EditLlmModelDrawerSet = StoreSet<EditLlmModelDrawerState>
export type EditLlmModelDrawerGet = () => EditLlmModelDrawerState
