import { ApiClient } from '@/api-client'
import type { CreateAssistantRequest } from '@/api-client/types'
import type { UserAssistantsGet, UserAssistantsSet } from '../state'
import loadFactory from './loadUserAssistants'
import { emitAssistantCreated } from '@/modules/assistant/events'

export default (set: UserAssistantsSet, get: UserAssistantsGet) => {
  const load = loadFactory(set, get)
  return async (data: CreateAssistantRequest) => {
    set(s => {
      s.creating = true
      s.error = null
    })
    try {
      const assistant = await ApiClient.Assistant.create(data)
      try {
        await emitAssistantCreated(assistant)
      } catch (eventError) {
        console.error('Failed to emit assistant created event:', eventError)
      }
      await load()
      set(s => {
        s.creating = false
      })
      return assistant
    } catch (error) {
      set(s => {
        s.error =
          error instanceof Error
            ? error.message
            : 'Failed to create assistant'
        s.creating = false
      })
      throw error
    }
  }
}
