import type { BaseEvent } from '@ziee/framework/events'
import type { AgentAdminSettings } from '@/api-client/types'

export interface AgentAdminSettingsUpdatedEvent extends BaseEvent {
  type: 'agent.admin_settings_updated'
  data: { settings: AgentAdminSettings }
}

export type AgentModuleEvent = AgentAdminSettingsUpdatedEvent

declare module '@ziee/framework/events' {
  interface AppEvents {
    'agent.admin_settings_updated': AgentAdminSettingsUpdatedEvent
  }
}
