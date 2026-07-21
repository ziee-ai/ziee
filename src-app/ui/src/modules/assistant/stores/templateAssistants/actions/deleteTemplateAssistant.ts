import { ApiClient } from '@/api-client'
import type { TemplateAssistantsGet, TemplateAssistantsSet } from '../state'
import loadTemplateAssistantsFactory from './loadTemplateAssistants'
import { emitAssistantTemplateDeleted } from '@/modules/assistant/events'

export default (set: TemplateAssistantsSet, get: TemplateAssistantsGet) => {
  const loadTemplateAssistants = loadTemplateAssistantsFactory(set, get)
  return async (id: string): Promise<void> => {
    if (get().deleting) return
    try {
      set({ deleting: true, error: null })
      await ApiClient.AssistantTemplate.delete({ id })
      try {
        await emitAssistantTemplateDeleted(id)
      } catch (eventError) {
        console.error('Failed to emit assistant template deleted event:', eventError)
      }
      await loadTemplateAssistants()
      set({ deleting: false })
    } catch (error) {
      set({
        error:
          error instanceof Error ? error.message : 'Failed to delete template assistant',
        deleting: false,
      })
      throw error
    }
  }
}
