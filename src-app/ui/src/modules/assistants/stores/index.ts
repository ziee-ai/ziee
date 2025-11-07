// Only export hooks, not action functions
export { useUserAssistantsStore } from './user-assistants-store'
export { useTemplateAssistantsStore } from './template-assistants-store'

// Re-export for compatibility with Stores pattern
export { Stores } from '@/core/stores'
