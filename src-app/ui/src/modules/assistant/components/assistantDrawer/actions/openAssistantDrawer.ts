import type { Assistant } from '@/api-client/types'
import type { AssistantDrawerGet, AssistantDrawerSet } from '../state'

export default (set: AssistantDrawerSet, _get: AssistantDrawerGet) =>
  async (
    assistant?: Assistant | null,
    isTemplate = false,
    isCloning = false,
  ) => {
    set({ open: true, editingAssistant: assistant || null, isTemplate, isCloning })
  }
