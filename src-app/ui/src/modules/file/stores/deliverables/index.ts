import { defineStore, registerLazyStore } from '@ziee/framework/store-kit'
import { deliverablesState, type DeliverablesState } from './state'
import type { Actions } from './actions.gen'

const DeliverablesDef = defineStore<DeliverablesState, Actions>('Deliverables', {
  immer: true,
  state: deliverablesState,
  actions: import.meta.glob('./actions/*.ts'),
  init: ({ on, get, actions }) => {
    // sync:deliverable — pin/unpin on this or another device → refetch.
    on('sync:deliverable', (event: { data?: { id?: string } }) => {
      const id = event?.data?.id
      if (id && get().byConversation.has(id)) void actions.load(id)
    })
    // Reconnect: we may have missed events → reload every tracked conversation.
    on('sync:reconnect', () => {
      Array.from(get().byConversation.keys()).forEach(
        id => void actions.load(id),
      )
    })
  },
})
export const Deliverables = registerLazyStore(DeliverablesDef)
export const useDeliverablesStore = DeliverablesDef.store
