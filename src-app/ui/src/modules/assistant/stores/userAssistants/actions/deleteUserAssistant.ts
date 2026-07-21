import { ApiClient } from '@/api-client'
import type { UserAssistantsGet, UserAssistantsSet } from '../state'
import loadFactory from './loadUserAssistants'
import { emitAssistantDeleted } from '@/modules/assistant/events'

export default (set: UserAssistantsSet, get: UserAssistantsGet) => {
  const load = loadFactory(set, get)
  return async (id: string) => {
    set(s => {
      s.deleting = true
      s.error = null
    })
    try {
      await ApiClient.Assistant.delete({ id })
      try {
        await emitAssistantDeleted(id)
      } catch (eventError) {
        console.error('Failed to emit assistant deleted event:', eventError)
      }
      await load()
      set(s => {
        s.deleting = false
      })
    } catch (error) {
      set(s => {
        s.error =
          error instanceof Error
            ? error.message
            : 'Failed to delete assistant'
        s.deleting = false
      })
      throw error
    }
  }
}
