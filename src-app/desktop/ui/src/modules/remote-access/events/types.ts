import type { BaseEvent } from '@/core/events'

export interface RemoteAccessStatusChangedEvent extends BaseEvent {
  type: 'remote_access.status_changed'
  data: { reason: 'settings' | 'tunnel' }
}

declare module '@/core/events' {
  interface AppEvents {
    'remote_access.status_changed': RemoteAccessStatusChangedEvent
  }
}
