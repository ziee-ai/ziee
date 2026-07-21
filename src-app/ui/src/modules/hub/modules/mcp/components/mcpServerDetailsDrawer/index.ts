import { defineStore, registerLazyStore } from '@ziee/framework/store-kit'
import { mcpServerDetailsDrawerState, type McpServerDetailsDrawerState } from './state'
import type { Actions } from './actions.gen'

const McpServerDetailsDrawerDef = defineStore<McpServerDetailsDrawerState, Actions>('McpServerDetailsDrawer', {
  immer: true,
  state: mcpServerDetailsDrawerState,
  actions: import.meta.glob('./actions/*.ts'),
})

export const McpServerDetailsDrawer = registerLazyStore(McpServerDetailsDrawerDef)
export const useMcpServerDetailsDrawerStore = McpServerDetailsDrawerDef.store
