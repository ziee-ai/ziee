import { defineStore } from '@ziee/framework/store-kit'
import {
  editLlmModelDrawerState,
  type EditLlmModelDrawerState,
} from './state'
import type { Actions } from './actions.gen'

export const EditLlmModelDrawer = defineStore<
  EditLlmModelDrawerState,
  Actions
>('EditLlmModelDrawer', {
  state: editLlmModelDrawerState,
  actions: import.meta.glob('./actions/*.ts'),
  init: ({ on, get, actions }) => {
    on('llm_model.deleted', event => {
      if (get().modelId === event.data.modelId) actions.closeEditLlmModelDrawer()
    })
  },
})
export const useEditLlmModelDrawerStore = EditLlmModelDrawer.store
