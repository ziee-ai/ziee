import { Permissions } from '@/api-client/permissions'
import { hasPermissionNow } from '@/core/permissions'
import type { SkillGet, SkillSet } from '../state'
import loadSkillsFactory from './_loadSkills'

export default (set: SkillSet, get: SkillGet) => {
  const doLoadSkills = loadSkillsFactory(set, get)
  return async () => {
    if (!hasPermissionNow(Permissions.SkillsRead)) return
    if (get().loading) return
    await doLoadSkills()
  }
}
