import { ApiClient } from '@/api-client'
import type { VoiceRuntimeVersionGet, VoiceRuntimeVersionSet } from '../state'

export default (set: VoiceRuntimeVersionSet, _get: VoiceRuntimeVersionGet) =>
  async (id: string, removeBinary = false) => {
    set(state => ({
      deleting: new Map(state.deleting).set(id, true),
      error: null,
    }))
    try {
      await ApiClient.Voice.deleteVersion({ id, remove_binary: removeBinary })
      set(state => {
        const newMap = new Map(state.deleting)
        newMap.delete(id)
        return { versions: state.versions.filter(v => v.id !== id), deleting: newMap }
      })
    } catch (error) {
      set(state => {
        const newMap = new Map(state.deleting)
        newMap.delete(id)
        return {
          deleting: newMap,
          error: error instanceof Error ? error.message : 'Failed to delete version',
        }
      })
      throw error
    }
  }
