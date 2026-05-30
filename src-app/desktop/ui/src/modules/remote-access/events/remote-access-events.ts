/**
 * Remote Access event definitions.
 *
 * Mutations emit these so other parts of the desktop UI (notifications,
 * cross-tab cache invalidation if/when we add it) can react.
 *
 * Event TYPES are declared in `./types.ts` via TypeScript declaration
 * merging into `@/core/events::AppEvents`. Importing this file is
 * also what brings that declaration into the project's type universe.
 */

import { Stores } from '@/core/stores'
import './types'

export const emitRemoteAccessStatusChanged = async (
  reason: 'settings' | 'tunnel',
) => {
  await Stores.EventBus.emit({
    type: 'remote_access.status_changed',
    data: { reason },
  })
}
