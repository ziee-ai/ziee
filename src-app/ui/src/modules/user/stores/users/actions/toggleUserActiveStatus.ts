import { ApiClient } from '@/api-client'
import { emitUserUpdated } from '@/modules/user/events'
import type { UsersSet, UsersGet } from '../state'

export default (set: UsersSet, get: UsersGet) =>
  async (id: string): Promise<void> => {
    if (get().updating) return
    try {
      set({ updating: true, error: null })
      await ApiClient.User.toggleActive({ user_id: id })
      const currentUser = get().users.find(u => u.id === id)
      if (currentUser) {
        const updatedUser = { ...currentUser, is_active: !currentUser.is_active }
        // Event handler updates state (no manual state update here).
        try {
          await emitUserUpdated(updatedUser)
        } catch (eventError) {
          console.error('Failed to emit user updated event:', eventError)
        }
      }
      set({ updating: false })
    } catch (error) {
      set({
        error: error instanceof Error ? error.message : 'Failed to toggle user status',
        updating: false,
      })
      throw error
    }
  }
