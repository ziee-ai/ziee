import { defineStore, registerLazyStore } from '@ziee/framework/store-kit'
import { memorySetupStepState, type MemorySetupStepState } from './state'
import type { Actions } from './actions.gen'

const MemorySetupStepDef = defineStore<MemorySetupStepState, Actions>('MemorySetupStep', {
  immer: true,
  state: memorySetupStepState,
  actions: import.meta.glob('./actions/*.ts'),
})

export const MemorySetupStep = registerLazyStore(MemorySetupStepDef)
export const useMemorySetupStepStore = MemorySetupStepDef.store

// Raw store for direct access (Stores proxy uses this).
export { MemorySetupStepDef }
