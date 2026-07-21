import { ApiClient } from '@/api-client'
import { emitRuntimeVersionDefaultChanged } from '../../../events/emitters'
import type { RuntimeVersionGet, RuntimeVersionSet } from '../state'

export default (set: RuntimeVersionSet, _get: RuntimeVersionGet) =>
  async (versionId: string) => {
    set(s => {
      const newMap = new Map(s.settingDefault)
      newMap.set(versionId, true)
      s.settingDefault = newMap
      s.error = null
    })
    try {
      await ApiClient.RuntimeVersion.setDefault({ version_id: versionId })
      await emitRuntimeVersionDefaultChanged(versionId)
      set(s => {
        const version = s.versions.find(v => v.id === versionId)
        if (!version) return
        s.versions = s.versions.map(v => ({
          ...v,
          is_system_default:
            v.engine === version.engine ? v.id === versionId : v.is_system_default,
        }))
        const newMap = new Map(s.settingDefault)
        newMap.delete(versionId)
        s.settingDefault = newMap
      })
    } catch (error) {
      set(s => {
        const newMap = new Map(s.settingDefault)
        newMap.delete(versionId)
        s.settingDefault = newMap
        s.error = error instanceof Error ? error.message : 'Failed to set default'
      })
      throw error
    }
  }
