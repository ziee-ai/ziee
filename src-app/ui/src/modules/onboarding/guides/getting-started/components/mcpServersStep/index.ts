import { defineStore, registerLazyStore } from '@ziee/framework/store-kit'
import { mcpServersStepState, type McpServersStepState } from './state'
import type { Actions } from './actions.gen'

const McpServersStepDef = defineStore<McpServersStepState, Actions>('McpServersStep', {
  immer: true,
  state: mcpServersStepState,
  actions: import.meta.glob('./actions/*.ts'),
})

export const McpServersStep = registerLazyStore(McpServersStepDef)
export const useMcpServersStepStore = McpServersStepDef.store

// Raw store for direct access (Stores proxy uses this).
export { McpServersStepDef }
