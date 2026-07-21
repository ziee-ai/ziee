import { ApiClient } from '@/api-client'
import type { CitationsGet, CitationsSet } from '../state'
import loadEntriesFactory from './_loadEntries'

export default (set: CitationsSet, get: CitationsGet) => {
  const loadEntries = loadEntriesFactory(set, get)
  return async () => {
    set(s => {
      s.verifying = true
      s.error = null
    })
    try {
      const pid = get().projectId
      const report = await ApiClient.Citations.reverify(pid ? { project_id: pid } : {})
      await loadEntries()
      set(s => {
        s.verifying = false
      })
      return report
    } catch (error) {
      set(s => {
        s.error = error instanceof Error ? error.message : 'Verify failed'
        s.verifying = false
      })
      throw error
    }
  }
}
