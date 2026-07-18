import { Stores } from '@ziee/framework/stores'
import type { User } from '@/api-client/types'

export const emitProfileUpdated = async (user: User) => {
  await Stores.EventBus.emit({ type: 'profile.updated', data: { user } })
}
