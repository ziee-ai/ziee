import { ApiClient } from '@/api-client'
import loadVersionsFactory from './loadVersions'
import type { RuntimeVersionGet, RuntimeVersionSet } from '../state'

export default (set: RuntimeVersionSet, get: RuntimeVersionGet) =>
  async () => {
    const loadVersions = loadVersionsFactory(set, get)
    set(s => {
      s.loading = true
      s.error = null
    })
    try {
      await ApiClient.RuntimeVersion.syncCache()
      await loadVersions() // Reload after sync
    } catch (error) {
      set(s => {
        s.error = error instanceof Error ? error.message : 'Failed to sync cache'
        s.loading = false
      })
    }
  }
