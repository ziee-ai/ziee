import { defineStore } from '@ziee/framework/store-kit'
import {
  addLocalLlmModelUploadDrawerState,
  type AddLocalLlmModelUploadDrawerState,
} from './state'
import type { Actions } from './actions.gen'

export const AddLocalLlmModelUploadDrawer = defineStore<
  AddLocalLlmModelUploadDrawerState,
  Actions
>('AddLocalLlmModelUploadDrawer', {
  state: addLocalLlmModelUploadDrawerState,
  actions: import.meta.glob('./actions/*.ts'),
})
export const useAddLocalLlmModelUploadDrawerStore =
  AddLocalLlmModelUploadDrawer.store
