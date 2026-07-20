import { ApiClient } from '@/api-client'
import { type UpdateUserRequest, type User } from '@/api-client/types'
import { emitUserUpdated } from '@/modules/user/events'
import type { UsersSet, UsersGet } from '../state'

export default (set: UsersSet, get: UsersGet) =>
  async (id: string, data: UpdateUserRequest): Promise<User | undefined> => {
    if (get().updating) return
    try {
      set({ updating: true, error: null })
      const user = await ApiClient.User.update({ user_id: id, ...data })
      // Event handler updates state (no manual state update here).
      try {
        await emitUserUpdated(user)
      } catch (eventError) {
        console.error('Failed to emit user updated event:', eventError)
      }
      set({ updating: false })
      return user
    } catch (error) {
      set({
        error: error instanceof Error ? error.message : 'Failed to update user',
        updating: false,
      })
      throw error
    }
  }
