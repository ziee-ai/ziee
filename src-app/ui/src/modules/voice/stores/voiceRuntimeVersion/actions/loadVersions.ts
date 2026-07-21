import { Permissions } from '@/api-client/permissions'
import { hasPermissionNow } from '@/core/permissions'
import type { VoiceRuntimeVersionGet, VoiceRuntimeVersionSet } from '../state'
import doLoadVersionsFactory from './_doLoadVersions'

export default (set: VoiceRuntimeVersionSet, get: VoiceRuntimeVersionGet) => {
  const doLoadVersions = doLoadVersionsFactory(set, get)
  return async () => {
    if (!hasPermissionNow(Permissions.VoiceAdminRead)) return
    await doLoadVersions()
  }
}
