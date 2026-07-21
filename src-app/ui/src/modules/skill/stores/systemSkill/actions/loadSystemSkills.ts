import { ApiClient } from '@/api-client'
import { Permissions } from '@/api-client/permissions'
import { hasPermissionNow } from '@/core/permissions'
import type { SystemSkillGet, SystemSkillSet } from '../state'

export default (set: SystemSkillSet, _get: SystemSkillGet) => async () => {
  if (!hasPermissionNow(Permissions.SkillsManageSystem)) return
  const get = () => _get()
  if (get().loading) return
  try {
    set(draft => {
      draft.loading = true
      draft.error = null
    })
    const response = await ApiClient.SkillSystem.list({})
    set(draft => {
      draft.systemSkills = response.skills
      draft.isInitialized = true
      draft.loading = false
    })
  } catch (error) {
    set(draft => {
      draft.loading = false
      draft.error =
        error instanceof Error ? error.message : 'Failed to load system skills'
    })
  }
}
