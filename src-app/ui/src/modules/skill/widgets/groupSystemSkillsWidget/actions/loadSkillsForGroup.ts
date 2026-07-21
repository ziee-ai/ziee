import { ApiClient } from '@/api-client'
import type { Skill } from '@/api-client/types'
import type { GroupSystemSkillsWidgetGet, GroupSystemSkillsWidgetSet, GroupSkills } from '../state'

export default (set: GroupSystemSkillsWidgetSet, get: GroupSystemSkillsWidgetGet) =>
  async (groupId: string, force = false) => {
    const existing = get().groupSkills.get(groupId)
    if (existing?.loading && !force) return
    if (!force && existing?.lastFetched && Date.now() - existing.lastFetched < 30000 && !existing.error) {
      return
    }

    const makePending = (prev: GroupSkills | undefined): GroupSkills => ({
      groupId,
      skills: prev?.skills ?? [],
      loading: true,
      error: null,
      lastFetched: prev?.lastFetched ?? null,
    })

    const makeSuccess = (skills: Skill[]): GroupSkills => ({
      groupId,
      skills,
      loading: false,
      error: null,
      lastFetched: Date.now(),
    })

    const makeError = (prev: GroupSkills | undefined, error: unknown): GroupSkills => ({
      groupId,
      skills: prev?.skills ?? [],
      loading: false,
      error: error instanceof Error ? error.message : 'Failed to load skills',
      lastFetched: prev?.lastFetched ?? null,
    })

    set(state => {
      state.groupSkills.set(groupId, makePending(state.groupSkills.get(groupId)))
    })

    try {
      const response = await ApiClient.Group.getSystemSkills({ group_id: groupId })
      set(state => {
        state.groupSkills.set(groupId, makeSuccess(response.skills))
      })
    } catch (error) {
      console.error(`Failed to load skills for group ${groupId}:`, error)
      set(state => {
        state.groupSkills.set(groupId, makeError(state.groupSkills.get(groupId), error))
      })
    }
  }
