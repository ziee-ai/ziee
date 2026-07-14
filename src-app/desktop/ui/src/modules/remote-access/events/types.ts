import type { BaseEvent } from '@ziee/framework/events'

export interface RemoteAccessStatusChangedEvent extends BaseEvent {
  type: 'remote_access.status_changed'
  data: { reason: 'settings' | 'tunnel' }
}

declare module '@ziee/framework/events' {
  interface AppEvents {
    'remote_access.status_changed': RemoteAccessStatusChangedEvent
  }
}
