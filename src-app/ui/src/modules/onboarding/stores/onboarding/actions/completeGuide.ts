import { ApiClient } from '@/api-client'
import type { OnboardingGet, OnboardingSet } from '../state'
import '@/modules/onboarding/events/types'
import { EventBus } from '@ziee/framework/stores'

export default (set: OnboardingSet, _get: OnboardingGet) =>
  async (guideId: string) => {
    const progress = await ApiClient.Onboarding.complete({ guide_id: guideId }, undefined)
    set(draft => {
      draft.completedGuideIds = progress.completed_guide_ids
      draft.completedStepIds = progress.completed_step_ids
    })
    await EventBus.emit({ type: 'onboarding.guide_completed', data: { guideId } })
  }
