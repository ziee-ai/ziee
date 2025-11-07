import { Stores } from '@/core/stores'
import type { Group, User } from '@/api-client/types'

/**
 * Emit group created event
 */
export const emitGroupCreated = async (group: Group) => {
  await Stores.EventBus.emit({
    type: 'group.created',
    data: { group },
  })
}

/**
 * Emit group updated event
 */
export const emitGroupUpdated = async (group: Group) => {
  await Stores.EventBus.emit({
    type: 'group.updated',
    data: { group },
  })
}

/**
 * Emit group deleted event
 */
export const emitGroupDeleted = async (groupId: string) => {
  await Stores.EventBus.emit({
    type: 'group.deleted',
    data: { groupId },
  })
}

/**
 * Emit user created event
 */
export const emitUserCreated = async (user: User) => {
  await Stores.EventBus.emit({
    type: 'user.created',
    data: { user },
  })
}

/**
 * Emit user updated event
 */
export const emitUserUpdated = async (user: User) => {
  await Stores.EventBus.emit({
    type: 'user.updated',
    data: { user },
  })
}
