import type { BaseEvent } from '@/core/events'
import type { User } from '@/api-client/types'

export interface OnboardingUserUpdatedEvent extends BaseEvent {
  type: 'onboarding.user_updated'
  data: { user: User }
}

declare module '@/core/events' {
  interface AppEvents {
    'onboarding.user_updated': OnboardingUserUpdatedEvent
  }
}
