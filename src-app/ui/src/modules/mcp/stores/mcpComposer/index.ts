import { enableMapSet } from 'immer'
import { defineStore, registerLazyStore } from '@ziee/framework/store-kit'
import {
  mcpComposerState,
  type McpComposerState,
} from './state'
import type { Actions } from './actions.gen'

// Re-export state types and helper constants so existing importers get them
// from the single `mcpComposer` barrel without needing sub-path imports.
export {
  mcpComposerState,
  type McpComposerState,
  type McpComposerSet,
  type McpComposerGet,
  type McpToolCall,
  type McpToolProgressState,
  type ElicitationRequestState,
  projectConfigKey,
  pendingConversationKey,
  approvalKeyOf,
  PENDING_CONVERSATION_KEY,
} from './state'

enableMapSet()

const McpComposerDef = defineStore<McpComposerState, Actions>('McpComposer', {
  immer: true,
  state: mcpComposerState,
  actions: import.meta.glob('./actions/*.ts', { eager: true }),
  init: ({ on, actions }) => {
    // Cross-device sync of the user's own MCP defaults. `loadUserDefaults`
    // self-gates on `conversations::read` internally (returns early when the
    // user lacks it), satisfying the no-403 reconnect rule — `sync:reconnect`
    // fires for every store on reconnect regardless of audience.
    const reload = () => void actions.loadUserDefaults()
    on('sync:mcp_defaults', reload)
    on('sync:reconnect', reload)
    void actions.loadUserDefaults()
  },
})
export const McpComposer = registerLazyStore(McpComposerDef)
export const useMcpComposerStore = McpComposerDef.store
