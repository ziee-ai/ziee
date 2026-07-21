import { ApiClient } from '@/api-client'
import type { UpdateAssistantRequest } from '@/api-client/types'
import type { UserAssistantsGet, UserAssistantsSet } from '../state'
import loadFactory from './loadUserAssistants'
import { emitAssistantUpdated } from '@/modules/assistant/events'

export default (set: UserAssistantsSet, get: UserAssistantsGet) => {
  const load = loadFactory(set, get)
  return async (id: string, data: UpdateAssistantRequest) => {
    set(s => {
      s.updating = true
      s.error = null
    })
    try {
      const assistant = await ApiClient.Assistant.update({ id, ...data })
      try {
        await emitAssistantUpdated(assistant)
      } catch (eventError) {
        console.error('Failed to emit assistant updated event:', eventError)
      }
      await load()
      set(s => {
        s.updating = false
      })
      return assistant
    } catch (error) {
      set(s => {
        s.error =
          error instanceof Error
            ? error.message
            : 'Failed to update assistant'
        s.updating = false
      })
      throw error
    }
  }
}
