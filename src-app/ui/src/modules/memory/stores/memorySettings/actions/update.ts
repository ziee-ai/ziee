import type { UserMemorySettings } from '@/api-client/types'
import { emitMemorySettingsUpdated } from '@/modules/memory/events'
import type { MemorySettingsGet, MemorySettingsSet } from '../state'
import doUpdateFactory from './_doUpdate'
import type { MemorySettingsUpdatePatch } from './_doUpdate'

export default (set: MemorySettingsSet, get: MemorySettingsGet) => {
  const doUpdate = doUpdateFactory(set, get)
  return async (patch: MemorySettingsUpdatePatch): Promise<
    UserMemorySettings
  > => {
    const row = await doUpdate(patch)
    try {
      await emitMemorySettingsUpdated(row)
    } catch (eventError) {
      console.error('Failed to emit memory settings updated event:', eventError)
    }
    return row
  }
}
