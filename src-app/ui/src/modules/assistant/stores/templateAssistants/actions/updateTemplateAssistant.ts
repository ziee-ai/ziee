import { ApiClient } from '@/api-client'
import type { UpdateAssistantRequest, Assistant } from '@/api-client/types'
import type { TemplateAssistantsGet, TemplateAssistantsSet } from '../state'
import loadTemplateAssistantsFactory from './loadTemplateAssistants'
import { emitAssistantTemplateUpdated } from '@/modules/assistant/events'

export default (set: TemplateAssistantsSet, get: TemplateAssistantsGet) => {
  const loadTemplateAssistants = loadTemplateAssistantsFactory(set, get)
  return async (id: string, data: UpdateAssistantRequest): Promise<Assistant | undefined> => {
    if (get().updating) return
    try {
      set({ updating: true, error: null })
      const assistant = await ApiClient.AssistantTemplate.update({ id, ...data })
      try {
        await emitAssistantTemplateUpdated(assistant)
      } catch (eventError) {
        console.error('Failed to emit assistant template updated event:', eventError)
      }
      await loadTemplateAssistants()
      set({ updating: false })
      return assistant
    } catch (error) {
      set({
        error:
          error instanceof Error ? error.message : 'Failed to update template assistant',
        updating: false,
      })
      throw error
    }
  }
}
