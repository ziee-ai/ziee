import type { BaseEvent } from '@/core/events'
import type { User } from '@/api-client/types'

export interface ProfileUpdatedEvent extends BaseEvent {
  type: 'profile.updated'
  data: { user: User }
}

export type ProfileModuleEvent = ProfileUpdatedEvent

declare module '@/core/events' {
  interface AppEvents {
    'profile.updated': ProfileUpdatedEvent
  }
}
