import { defineStore, registerLazyStore } from '@ziee/framework/store-kit'
import { llmModelUploadState, type LlmModelUploadState } from './state'
import type { Actions } from './actions.gen'

const LlmModelUploadDef = defineStore<LlmModelUploadState, Actions>('LlmModelUpload', {
  immer: true,
  state: llmModelUploadState,
  actions: import.meta.glob('./actions/*.ts'),
})
export const LlmModelUpload = registerLazyStore(LlmModelUploadDef)
export const useUploadStore = LlmModelUploadDef.store
