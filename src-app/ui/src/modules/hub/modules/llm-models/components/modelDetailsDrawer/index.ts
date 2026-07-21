import { defineStore, registerLazyStore } from '@ziee/framework/store-kit'
import { modelDetailsDrawerState, type ModelDetailsDrawerState } from './state'
import type { Actions } from './actions.gen'

const ModelDetailsDrawerDef = defineStore<ModelDetailsDrawerState, Actions>(
  'ModelDetailsDrawer',
  {
    immer: true,
    state: modelDetailsDrawerState,
    actions: import.meta.glob('./actions/*.ts'),
  },
)

export const ModelDetailsDrawer = registerLazyStore(ModelDetailsDrawerDef)
export const useModelDetailsDrawerStore = ModelDetailsDrawerDef.store
