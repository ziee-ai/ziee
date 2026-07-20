import { ApiClient } from '@/api-client'
import { type CreateUserRequest, type User } from '@/api-client/types'
import { emitUserCreated } from '@/modules/user/events'
import type { UsersSet, UsersGet } from '../state'

export default (set: UsersSet, get: UsersGet) =>
  async (data: CreateUserRequest): Promise<User | undefined> => {
    if (get().creating) return
    try {
      set({ creating: true, error: null })
      const user = await ApiClient.User.create(data)
      try {
        await emitUserCreated(user)
      } catch (eventError) {
        console.error('Failed to emit user created event:', eventError)
      }
      set(state => ({
        users: [...state.users, user],
        total: state.total + 1,
        creating: false,
      }))
      return user
    } catch (error) {
      set({
        error: error instanceof Error ? error.message : 'Failed to create user',
        creating: false,
      })
      throw error
    }
  }
