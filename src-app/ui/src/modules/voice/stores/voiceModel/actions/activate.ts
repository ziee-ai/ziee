import { ApiClient } from '@/api-client'
import type { VoiceModelGet, VoiceModelSet } from '../state'

export default (set: VoiceModelSet, _get: VoiceModelGet) =>
  async (id: string) => {
    set(state => ({
      activating: new Map(state.activating).set(id, true),
      error: null,
    }))
    try {
      const updated = await ApiClient.Voice.activateModel({ id })
      set(state => {
        const next = new Map(state.activating)
        next.delete(id)
        return {
          activating: next,
          installed: state.installed.map(m => ({
            ...m,
            is_active: m.id === updated.id,
          })),
        }
      })
    } catch (error) {
      set(state => {
        const next = new Map(state.activating)
        next.delete(id)
        return {
          activating: next,
          error:
            error instanceof Error
              ? error.message
              : 'Failed to activate model',
        }
      })
      throw error
    }
  }
