import { ApiClient } from '@/api-client'
import type { CitationsSet } from '../state'

export default (set: CitationsSet, _get: () => never) => {
  return async (id: string) => {
    try {
      await ApiClient.Citations.delete({ id })
      set(s => {
        s.entries = s.entries.filter(e => e.id !== id)
      })
    } catch (error) {
      set(s => {
        s.error = error instanceof Error ? error.message : 'Delete failed'
      })
      throw error
    }
  }
}
