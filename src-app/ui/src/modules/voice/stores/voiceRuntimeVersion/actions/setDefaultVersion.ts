import { ApiClient } from '@/api-client'
import type { VoiceRuntimeVersionGet, VoiceRuntimeVersionSet } from '../state'

export default (set: VoiceRuntimeVersionSet, _get: VoiceRuntimeVersionGet) =>
  async (id: string) => {
    set(state => ({
      settingDefault: new Map(state.settingDefault).set(id, true),
      error: null,
    }))
    try {
      await ApiClient.Voice.setDefaultVersion({ id })
      set(state => {
        const versions = state.versions.map(v => ({
          ...v,
          is_system_default: v.id === id,
        }))
        const newMap = new Map(state.settingDefault)
        newMap.delete(id)
        return { versions, settingDefault: newMap }
      })
    } catch (error) {
      set(state => {
        const newMap = new Map(state.settingDefault)
        newMap.delete(id)
        return {
          settingDefault: newMap,
          error: error instanceof Error ? error.message : 'Failed to set default',
        }
      })
      throw error
    }
  }
