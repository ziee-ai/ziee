import { ApiClient } from '@/api-client'
import { emitUserUpdated } from '@/modules/user/events'
import type { UsersSet, UsersGet } from '../state'

export default (set: UsersSet, get: UsersGet) =>
  async (id: string, newPassword: string): Promise<void> => {
    if (get().updating) return
    try {
      set({ updating: true, error: null })
      await ApiClient.User.resetPassword({ user_id: id, new_password: newPassword })
      const user = get().users.find(u => u.id === id)
      if (user) {
        try {
          await emitUserUpdated(user)
        } catch (eventError) {
          console.error('Failed to emit user updated event:', eventError)
        }
      }
      set({ updating: false })
    } catch (error) {
      set({
        error: error instanceof Error ? error.message : 'Failed to reset password',
        updating: false,
      })
      throw error
    }
  }
