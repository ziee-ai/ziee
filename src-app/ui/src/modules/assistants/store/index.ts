// Only export hooks, not action functions
export { useUserAssistantsStore } from './user-assistants-store'
export { useTemplateAssistantsStore } from './template-assistants-store'
export { useAssistantDrawerStore } from './assistant-drawer-store'

// Re-export for compatibility with Stores pattern
export { Stores } from '@/core/stores'
