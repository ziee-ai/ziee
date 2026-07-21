import type { Group, User } from '@/api-client/types'
import { EventBus } from '@ziee/framework/stores'

/**
 * Emit group created event
 */
export const emitGroupCreated = async (group: Group) => {
  await EventBus.emit({
    type: 'group.created',
    data: { group },
  })
}

/**
 * Emit group updated event
 */
export const emitGroupUpdated = async (group: Group) => {
  await EventBus.emit({
    type: 'group.updated',
    data: { group },
  })
}

/**
 * Emit group deleted event
 */
export const emitGroupDeleted = async (groupId: string) => {
  await EventBus.emit({
    type: 'group.deleted',
    data: { groupId },
  })
}

/**
 * Emit user created event
 */
export const emitUserCreated = async (user: User) => {
  await EventBus.emit({
    type: 'user.created',
    data: { user },
  })
}

/**
 * Emit user updated event
 */
export const emitUserUpdated = async (user: User) => {
  await EventBus.emit({
    type: 'user.updated',
    data: { user },
  })
}

/**
 * Emit user deleted event
 */
export const emitUserDeleted = async (userId: string) => {
  await EventBus.emit({
    type: 'user.deleted',
    data: { userId },
  })
}

/**
 * Emit group member added event
 */
export const emitGroupMemberAdded = async (groupId: string, userId: string) => {
  await EventBus.emit({
    type: 'group.member_added',
    data: { groupId, userId },
  })
}

/**
 * Emit group member removed event
 */
export const emitGroupMemberRemoved = async (
  groupId: string,
  userId: string,
) => {
  await EventBus.emit({
    type: 'group.member_removed',
    data: { groupId, userId },
  })
}
