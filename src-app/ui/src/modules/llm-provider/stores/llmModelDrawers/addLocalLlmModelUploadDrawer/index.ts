import { defineStore, registerLazyStore } from '@ziee/framework/store-kit'
import {
  addLocalLlmModelUploadDrawerState,
  type AddLocalLlmModelUploadDrawerState,
} from './state'
import type { Actions } from './actions.gen'

const AddLocalLlmModelUploadDrawerDef = defineStore<
  AddLocalLlmModelUploadDrawerState,
  Actions
>('AddLocalLlmModelUploadDrawer', {
  // Draft-mutation actions need immer (see addRemoteLlmModelDrawer note).
  immer: true,
  state: addLocalLlmModelUploadDrawerState,
  actions: import.meta.glob('./actions/*.ts'),
})
export const useAddLocalLlmModelUploadDrawerStore =
  AddLocalLlmModelUploadDrawerDef.store

export const AddLocalLlmModelUploadDrawer = registerLazyStore(AddLocalLlmModelUploadDrawerDef)
