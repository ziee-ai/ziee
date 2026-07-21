import { defineStore, registerLazyStore } from '@ziee/framework/store-kit'
import { llmModelDownloadState, type LlmModelDownloadState } from './state'
import type { Actions } from './actions.gen'

const LlmModelDownloadDef = defineStore<LlmModelDownloadState, Actions>('LlmModelDownload', {
  immer: true,
  state: llmModelDownloadState,
  actions: import.meta.glob('./actions/*.ts'),
  init: ({ actions, onCleanup }) => {
    void actions.initializeDownloadTracking()
    // Abort the module-scope SSE controller on store destroy. (audit 09 B-8)
    onCleanup(() => {
      const controller = (globalThis as Record<string, unknown>).__LLM_DL_SSE_ABORT as
        | AbortController
        | undefined
      if (controller) {
        controller.abort()
        ;(globalThis as Record<string, unknown>).__LLM_DL_SSE_ABORT = undefined
      }
    })
  },
})

// The raw Zustand store for gallery setup that needs direct setState.
export const LlmModelDownloadStore = LlmModelDownloadDef.store

export const LlmModelDownload = registerLazyStore(LlmModelDownloadDef)
export const useLlmModelDownloadStore = LlmModelDownloadDef.store
export type { LlmModelDownloadState }
