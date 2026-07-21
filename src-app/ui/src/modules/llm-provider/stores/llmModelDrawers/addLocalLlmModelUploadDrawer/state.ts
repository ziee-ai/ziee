import type { StoreSet } from '@ziee/framework/store-kit'

export const addLocalLlmModelUploadDrawerState = {
  open: false,
  loading: false,
  providerId: null as string | null,
}

export type AddLocalLlmModelUploadDrawerState = typeof addLocalLlmModelUploadDrawerState
export type AddLocalLlmModelUploadDrawerSet = StoreSet<AddLocalLlmModelUploadDrawerState>
export type AddLocalLlmModelUploadDrawerGet = () => AddLocalLlmModelUploadDrawerState
