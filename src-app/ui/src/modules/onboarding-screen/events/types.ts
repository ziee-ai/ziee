import type { BaseEvent } from '@/core/events'
import type { User } from '@/api-client/types'

export interface OnboardingScreenUserUpdatedEvent extends BaseEvent {
  type: 'onboarding_screen.user_updated'
  data: { user: User }
}

declare module '@/core/events' {
  interface AppEvents {
    'onboarding_screen.user_updated': OnboardingScreenUserUpdatedEvent
  }
}
