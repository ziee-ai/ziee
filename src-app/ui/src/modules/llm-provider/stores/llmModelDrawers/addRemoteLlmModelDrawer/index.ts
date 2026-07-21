import { defineStore, registerLazyStore } from '@ziee/framework/store-kit'
import {
  addRemoteLlmModelDrawerState,
  type AddRemoteLlmModelDrawerState,
} from './state'
import type { Actions } from './actions.gen'

const AddRemoteLlmModelDrawerDef = defineStore<
  AddRemoteLlmModelDrawerState,
  Actions
>('AddRemoteLlmModelDrawer', {
  state: addRemoteLlmModelDrawerState,
  actions: import.meta.glob('./actions/*.ts'),
})
export const useAddRemoteLlmModelDrawerStore = AddRemoteLlmModelDrawerDef.store

export const AddRemoteLlmModelDrawer = registerLazyStore(AddRemoteLlmModelDrawerDef)
