import type { User } from '@/api-client/types'
import { EventBus } from '@ziee/framework/stores'

export const emitProfileUpdated = async (user: User) => {
  await EventBus.emit({ type: 'profile.updated', data: { user } })
}
