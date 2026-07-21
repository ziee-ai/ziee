import { ApiClient } from '@/api-client'
import type { ValidateSkillResponse } from '@/api-client/types'
import type { SkillGet, SkillSet } from '../state'

export default (_set: SkillSet, _get: SkillGet) =>
  async (skillMd: string): Promise<ValidateSkillResponse> => {
    return await ApiClient.Skill.validate({ skill_md: skillMd })
  }
