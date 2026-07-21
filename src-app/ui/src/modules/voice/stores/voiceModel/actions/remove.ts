import { ApiClient } from '@/api-client'
import type { VoiceModelGet, VoiceModelSet } from '../state'

export default (set: VoiceModelSet, _get: VoiceModelGet) =>
  async (id: string, ackActive = false) => {
    set(state => ({
      deleting: new Map(state.deleting).set(id, true),
      error: null,
    }))
    try {
      await ApiClient.Voice.deleteModel({ id, ack_active: ackActive })
      set(state => {
        const next = new Map(state.deleting)
        next.delete(id)
        return {
          deleting: next,
          installed: state.installed.filter(m => m.id !== id),
        }
      })
    } catch (error) {
      set(state => {
        const next = new Map(state.deleting)
        next.delete(id)
        return {
          deleting: next,
          error:
            error instanceof Error ? error.message : 'Failed to delete model',
        }
      })
      throw error
    }
  }
