import type { BaseEvent } from '@ziee/framework/events'

export interface OnboardingGuideCompletedEvent extends BaseEvent {
  type: 'onboarding.guide_completed'
  data: { guideId: string }
}

export interface OnboardingStepCompletedEvent extends BaseEvent {
  type: 'onboarding.step_completed'
  data: { guideId: string; stepId: string }
}

declare module '@ziee/framework/events' {
  interface AppEvents {
    'onboarding.guide_completed': OnboardingGuideCompletedEvent
    'onboarding.step_completed': OnboardingStepCompletedEvent
  }
}
