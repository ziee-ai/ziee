import type { StoreSet } from '@ziee/framework/store-kit'

export const addLocalLlmModelDownloadDrawerState = {
  open: false,
  loading: false,
  providerId: null as string | null,
}

export type AddLocalLlmModelDownloadDrawerState = typeof addLocalLlmModelDownloadDrawerState
export type AddLocalLlmModelDownloadDrawerSet = StoreSet<AddLocalLlmModelDownloadDrawerState>
export type AddLocalLlmModelDownloadDrawerGet = () => AddLocalLlmModelDownloadDrawerState
