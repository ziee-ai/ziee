import { ApiClient } from '@/api-client'
import { Stores } from '@ziee/framework/stores'
import type { OnboardingGet, OnboardingSet } from '../state'
import '@/modules/onboarding/events/types'

export default (set: OnboardingSet, _get: OnboardingGet) =>
  async (guideId: string, stepId: string) => {
    const progress = await ApiClient.Onboarding.completeStep(
      { guide_id: guideId, step_id: stepId },
      undefined,
    )
    set(draft => {
      draft.completedGuideIds = progress.completed_guide_ids
      draft.completedStepIds = progress.completed_step_ids
    })
    await Stores.EventBus.emit({
      type: 'onboarding.step_completed',
      data: { guideId, stepId },
    })
  }
