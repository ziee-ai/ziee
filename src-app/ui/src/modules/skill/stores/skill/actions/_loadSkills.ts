import { ApiClient } from '@/api-client'
import type { SkillGet, SkillSet } from '../state'

/**
 * Internal: the raw fetch-and-set for skill loading.
 * Called directly by the public `loadSkills` (which also has a guard)
 * and by sync handlers.
 */
export default (set: SkillSet, _get: SkillGet) =>
  async () => {
    set(draft => {
      draft.loading = true
      draft.error = null
    })
    try {
      const response = await ApiClient.Skill.list({})
      set(draft => {
        draft.skills = response.skills
        draft.isInitialized = true
        draft.loading = false
      })
    } catch (error) {
      console.error('Skills loading failed:', error)
      set(draft => {
        draft.loading = false
        draft.error = error instanceof Error ? error.message : 'Failed to load skills'
      })
    }
  }
