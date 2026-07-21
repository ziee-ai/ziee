import { ApiClient } from '@/api-client'
import type { CreateAssistantRequest, Assistant } from '@/api-client/types'
import type { TemplateAssistantsGet, TemplateAssistantsSet } from '../state'
import loadTemplateAssistantsFactory from './loadTemplateAssistants'
import { emitAssistantTemplateCreated } from '@/modules/assistant/events'

export default (set: TemplateAssistantsSet, get: TemplateAssistantsGet) => {
  const loadTemplateAssistants = loadTemplateAssistantsFactory(set, get)
  return async (data: CreateAssistantRequest): Promise<Assistant | undefined> => {
    if (get().creating) return
    try {
      set({ creating: true, error: null })
      const assistant = await ApiClient.AssistantTemplate.create(data)
      try {
        await emitAssistantTemplateCreated(assistant)
      } catch (eventError) {
        console.error('Failed to emit assistant template created event:', eventError)
      }
      await loadTemplateAssistants()
      set({ creating: false })
      return assistant
    } catch (error) {
      set({
        error:
          error instanceof Error ? error.message : 'Failed to create template assistant',
        creating: false,
      })
      throw error
    }
  }
}
