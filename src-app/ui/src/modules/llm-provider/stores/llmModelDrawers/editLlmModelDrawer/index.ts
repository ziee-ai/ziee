import { defineStore, registerLazyStore } from '@ziee/framework/store-kit'
import {
  editLlmModelDrawerState,
  type EditLlmModelDrawerState,
} from './state'
import type { Actions } from './actions.gen'

const EditLlmModelDrawerDef = defineStore<
  EditLlmModelDrawerState,
  Actions
>('EditLlmModelDrawer', {
  // Draft-mutation actions need immer (see addRemoteLlmModelDrawer note).
  immer: true,
  state: editLlmModelDrawerState,
  actions: import.meta.glob('./actions/*.ts'),
  init: ({ on, get, actions }) => {
    on('llm_model.deleted', event => {
      if (get().modelId === event.data.modelId) actions.closeEditLlmModelDrawer()
    })
  },
})
export const useEditLlmModelDrawerStore = EditLlmModelDrawerDef.store

export const EditLlmModelDrawer = registerLazyStore(EditLlmModelDrawerDef)
