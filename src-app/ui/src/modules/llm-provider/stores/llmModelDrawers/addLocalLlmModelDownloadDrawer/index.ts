import { defineStore, registerLazyStore } from '@ziee/framework/store-kit'
import {
  addLocalLlmModelDownloadDrawerState,
  type AddLocalLlmModelDownloadDrawerState,
} from './state'
import type { Actions } from './actions.gen'

const AddLocalLlmModelDownloadDrawerDef = defineStore<
  AddLocalLlmModelDownloadDrawerState,
  Actions
>('AddLocalLlmModelDownloadDrawer', {
  // Draft-mutation actions need immer (see addRemoteLlmModelDrawer note).
  immer: true,
  state: addLocalLlmModelDownloadDrawerState,
  actions: import.meta.glob('./actions/*.ts'),
})
export const useAddLocalLlmModelDownloadDrawerStore =
  AddLocalLlmModelDownloadDrawerDef.store

export const AddLocalLlmModelDownloadDrawer = registerLazyStore(AddLocalLlmModelDownloadDrawerDef)
