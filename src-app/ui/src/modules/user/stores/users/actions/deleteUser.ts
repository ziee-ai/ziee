import { ApiClient } from '@/api-client'
import { emitUserDeleted } from '@/modules/user/events'
import type { UsersSet, UsersGet } from '../state'

export default (set: UsersSet, get: UsersGet) =>
  async (id: string): Promise<void> => {
    if (get().deleting) return
    try {
      set({ deleting: true, error: null })
      await ApiClient.User.delete({ user_id: id })
      // Event handler updates state (no manual state update here).
      try {
        await emitUserDeleted(id)
      } catch (eventError) {
        console.error('Failed to emit user deleted event:', eventError)
      }
      set({ deleting: false })
    } catch (error) {
      set({
        error: error instanceof Error ? error.message : 'Failed to delete user',
        deleting: false,
      })
      throw error
    }
  }
