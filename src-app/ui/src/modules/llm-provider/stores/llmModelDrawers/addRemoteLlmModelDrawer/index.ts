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
  // Actions use immer draft-mutation (`set(s => { s.open = true })`); without
  // this flag zustand v5 treats the mutate-and-return-undefined recipe as a
  // no-op, so `open` never flips and the drawer never mounts.
  immer: true,
  state: addRemoteLlmModelDrawerState,
  actions: import.meta.glob('./actions/*.ts'),
})
export const useAddRemoteLlmModelDrawerStore = AddRemoteLlmModelDrawerDef.store

export const AddRemoteLlmModelDrawer = registerLazyStore(AddRemoteLlmModelDrawerDef)
