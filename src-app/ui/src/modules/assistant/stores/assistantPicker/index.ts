import { defineStore, registerLazyStore } from '@ziee/framework/store-kit'
import { Permissions } from '@/api-client/permissions'
import { hasPermissionNow } from '@/core/permissions'
import {
  assistantPickerState,
  type AssistantPickerState,
  NEW_CHAT_ASSISTANT_KEY,
  newChatAssistantKey,
} from './state'
import type { Actions } from './actions.gen'

const AssistantPickerDef = defineStore<AssistantPickerState, Actions>(
  'AssistantPicker',
  {
    immer: true,
    state: assistantPickerState,
    actions: import.meta.glob('./actions/*'),
    init: ({ on, set, actions }) => {
      // Keep the cached picker list fresh on remote assistant create/edit/delete
      // (or reconnect). Self-gated on assistants::read (no-403 reconnect rule).
      const reload = () => {
        if (!hasPermissionNow(Permissions.AssistantsRead)) return
        void actions.loadAssistants(true)
      }
      on('sync:assistant', reload)
      on('sync:reconnect', reload)
      // Prune a deleted conversation's per-conversation assistant selection so the
      // `selectedByConversation` map doesn't grow unbounded / retain stale keys.
      on('sync:conversation', event => {
        if (event.data.action === 'delete') {
          set(state => {
            delete state.selectedByConversation[event.data.id]
          })
        }
      })
      void actions.loadAssistants()
    },
  },
)

export const AssistantPicker = registerLazyStore(AssistantPickerDef)
export const useAssistantPickerStore = AssistantPickerDef.store

// Re-export constants that callers import directly from the store file.
export { NEW_CHAT_ASSISTANT_KEY, newChatAssistantKey }
