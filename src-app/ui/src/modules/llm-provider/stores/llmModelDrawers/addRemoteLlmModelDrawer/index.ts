import { defineStore } from '@ziee/framework/store-kit'
import {
  addRemoteLlmModelDrawerState,
  type AddRemoteLlmModelDrawerState,
} from './state'
import type { Actions } from './actions.gen'

export const AddRemoteLlmModelDrawer = defineStore<
  AddRemoteLlmModelDrawerState,
  Actions
>('AddRemoteLlmModelDrawer', {
  state: addRemoteLlmModelDrawerState,
  actions: import.meta.glob('./actions/*.ts'),
})
export const useAddRemoteLlmModelDrawerStore = AddRemoteLlmModelDrawer.store
