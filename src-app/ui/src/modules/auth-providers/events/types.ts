import type { BaseEvent } from '@ziee/framework/events'
import type { AuthProviderResponse } from '@/api-client/types'

export interface AuthProviderCreatedEvent extends BaseEvent {
  type: 'auth_provider.created'
  data: {
    provider: AuthProviderResponse
  }
}

export interface AuthProviderUpdatedEvent extends BaseEvent {
  type: 'auth_provider.updated'
  data: {
    provider: AuthProviderResponse
  }
}

export interface AuthProviderDeletedEvent extends BaseEvent {
  type: 'auth_provider.deleted'
  data: {
    providerId: string
  }
}

/**
 * Emitted by the store when:
 *  - a create's `connection_warning` was populated (backend created the
 *    row enabled=true but the probe failed and the row was downgraded),
 *  - an update returned 400 `AUTH_PROVIDER_ENABLE_FAILED_HEALTH_CHECK`
 *    (backend reverted enabled to false),
 *  - a manual Test of an enabled row failed (backend auto-disabled).
 *
 * The settings page subscribes to reload the list so the Switch snaps
 * back + the failed-test Alert renders without a manual refresh.
 */
export interface AuthProviderAutoDisabledEvent extends BaseEvent {
  type: 'auth_provider.auto_disabled'
  data: {
    providerId: string
    reason: string
  }
}

export type AuthProviderModuleEvent =
  | AuthProviderCreatedEvent
  | AuthProviderUpdatedEvent
  | AuthProviderDeletedEvent
  | AuthProviderAutoDisabledEvent

declare module '@ziee/framework/events' {
  interface AppEvents {
    'auth_provider.created': AuthProviderCreatedEvent
    'auth_provider.updated': AuthProviderUpdatedEvent
    'auth_provider.deleted': AuthProviderDeletedEvent
    'auth_provider.auto_disabled': AuthProviderAutoDisabledEvent
  }
}
