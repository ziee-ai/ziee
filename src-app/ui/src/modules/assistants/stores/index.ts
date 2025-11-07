// Only export hooks, not action functions
export { useUserAssistantsStore } from './UserAssistants.store'
export { useTemplateAssistantsStore } from './TemplateAssistants.store'

// Re-export for compatibility with Stores pattern
export { Stores } from '@/core/stores'
