import { ApiClient } from '@/api-client'
import { emitRuntimeVersionDeleted } from '../../../events/emitters'
import type { RuntimeVersionGet, RuntimeVersionSet } from '../state'

export default (set: RuntimeVersionSet, _get: RuntimeVersionGet) =>
  async (versionId: string, removeBinary = false) => {
    set(s => {
      const newMap = new Map(s.deleting)
      newMap.set(versionId, true)
      s.deleting = newMap
      s.error = null
    })
    try {
      await ApiClient.RuntimeVersion.delete({
        version_id: versionId,
        remove_binary: removeBinary,
      })
      await emitRuntimeVersionDeleted(versionId)
      set(s => {
        s.versions = s.versions.filter(v => v.id !== versionId)
        const newMap = new Map(s.deleting)
        newMap.delete(versionId)
        s.deleting = newMap
      })
    } catch (error) {
      set(s => {
        const newMap = new Map(s.deleting)
        newMap.delete(versionId)
        s.deleting = newMap
        s.error = error instanceof Error ? error.message : 'Failed to delete version'
      })
      throw error
    }
  }
