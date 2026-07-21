import { defineStore } from '@ziee/framework/store-kit'
import {
  addLocalLlmModelDownloadDrawerState,
  type AddLocalLlmModelDownloadDrawerState,
} from './state'
import type { Actions } from './actions.gen'

export const AddLocalLlmModelDownloadDrawer = defineStore<
  AddLocalLlmModelDownloadDrawerState,
  Actions
>('AddLocalLlmModelDownloadDrawer', {
  state: addLocalLlmModelDownloadDrawerState,
  actions: import.meta.glob('./actions/*.ts'),
})
export const useAddLocalLlmModelDownloadDrawerStore =
  AddLocalLlmModelDownloadDrawer.store
