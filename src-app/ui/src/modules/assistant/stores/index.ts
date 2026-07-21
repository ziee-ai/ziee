// Only export hooks, not action functions
export { useUserAssistantsStore } from './UserAssistants.store'
export { useTemplateAssistantsStore } from './templateAssistants'
export { useAssistantPickerStore } from './assistantPicker'

// Re-export for compatibility with Stores pattern
export { Stores } from '@ziee/framework/stores'

// Re-export constants that callers import directly from the store.
export {
  NEW_CHAT_ASSISTANT_KEY,
  newChatAssistantKey,
} from './assistantPicker'
