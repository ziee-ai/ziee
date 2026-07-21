/**
 * Remote Access event definitions.
 *
 * Mutations emit these so other parts of the desktop UI (notifications,
 * cross-tab cache invalidation if/when we add it) can react.
 *
 * Event TYPES are declared in `./types.ts` via TypeScript declaration
 * merging into `@ziee/framework/events::AppEvents`. Importing this file is
 * also what brings that declaration into the project's type universe.
 */

import './types'
import { EventBus } from '@ziee/framework/stores'

export const emitRemoteAccessStatusChanged = async (
  reason: 'settings' | 'tunnel',
) => {
  await EventBus.emit({
    type: 'remote_access.status_changed',
    data: { reason },
  })
}
