import { Stores } from '@ziee/framework/stores'
import type { AuthProviderResponse } from '@/api-client/types'

export const emitAuthProviderCreated = async (
  provider: AuthProviderResponse,
) => {
  await Stores.EventBus.emit({
    type: 'auth_provider.created',
    data: { provider },
  })
}

export const emitAuthProviderUpdated = async (
  provider: AuthProviderResponse,
) => {
  await Stores.EventBus.emit({
    type: 'auth_provider.updated',
    data: { provider },
  })
}

export const emitAuthProviderDeleted = async (providerId: string) => {
  await Stores.EventBus.emit({
    type: 'auth_provider.deleted',
    data: { providerId },
  })
}

/**
 * Emitted from the store's create / update / test flows when the
 * backend's enforcement layer flipped a row to `enabled = false`
 * (either via `connection_warning` on create, the 400
 * `AUTH_PROVIDER_ENABLE_FAILED_HEALTH_CHECK` on update, or the manual
 * Test auto-disable). Triggers a list reload so the row's
 * `last_test_*` columns + the Switch state refresh in the visible DOM.
 */
export const emitAuthProviderAutoDisabled = async (
  providerId: string,
  reason: string,
) => {
  await Stores.EventBus.emit({
    type: 'auth_provider.auto_disabled',
    data: { providerId, reason },
  })
}
