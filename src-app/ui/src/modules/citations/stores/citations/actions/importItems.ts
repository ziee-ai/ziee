import { ApiClient } from '@/api-client'
import type { CitationInput } from '@/api-client/types'
import type { CitationsGet, CitationsSet } from '../state'
import loadEntriesFactory from './_loadEntries'

export default (set: CitationsSet, get: CitationsGet) => {
  const loadEntries = loadEntriesFactory(set, get)
  return async (items: CitationInput[], projectId?: string | null) => {
    set(s => {
      s.importing = true
      s.error = null
    })
    try {
      const pid = projectId !== undefined ? projectId : get().projectId
      const report = await ApiClient.Citations.import({
        items,
        ...(pid ? { project_id: pid } : {}),
      })
      await loadEntries()
      set(s => {
        s.importing = false
      })
      return report
    } catch (error) {
      set(s => {
        s.error = error instanceof Error ? error.message : 'Import failed'
        s.importing = false
      })
      throw error
    }
  }
}
