import type { BaseEvent } from '@/core/events'
import type { Group, User } from '@/api-client/types'

// Define module-specific events
export interface GroupCreatedEvent extends BaseEvent {
  type: 'group.created'
  data: {
    group: Group
  }
}

export interface GroupUpdatedEvent extends BaseEvent {
  type: 'group.updated'
  data: {
    group: Group
  }
}

export interface GroupDeletedEvent extends BaseEvent {
  type: 'group.deleted'
  data: {
    groupId: string
  }
}

export interface UserCreatedEvent extends BaseEvent {
  type: 'user.created'
  data: {
    user: User
  }
}

export interface UserUpdatedEvent extends BaseEvent {
  type: 'user.updated'
  data: {
    user: User
  }
}

// Union of all user module events
export type UserModuleEvent =
  | GroupCreatedEvent
  | GroupUpdatedEvent
  | GroupDeletedEvent
  | UserCreatedEvent
  | UserUpdatedEvent

// Register events in global registry via declaration merging
declare module '@/core/events' {
  interface AppEvents {
    'group.created': GroupCreatedEvent
    'group.updated': GroupUpdatedEvent
    'group.deleted': GroupDeletedEvent
    'user.created': UserCreatedEvent
    'user.updated': UserUpdatedEvent
  }
}
