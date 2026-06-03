// Only export hooks, not action functions
export { useUserAssistantsStore } from './UserAssistants.store'
export { useTemplateAssistantsStore } from './TemplateAssistants.store'
export { useAssistantPickerStore } from './AssistantPicker.store'

// Re-export for compatibility with Stores pattern
export { Stores } from '@/core/stores'
