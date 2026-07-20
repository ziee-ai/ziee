import type { UsersSet, UsersGet } from '../state'

export default (set: UsersSet, get: UsersGet) =>
  async (enabled: boolean): Promise<void> => {
    if (get().updating) return
    try {
      set({ updating: true, error: null })
      // TODO: Replace with actual API call when backend endpoint exists.
      set({ userRegistrationEnabled: enabled, updating: false })
    } catch (error) {
      set({
        error:
          error instanceof Error ? error.message : 'Failed to update registration settings',
        updating: false,
      })
      throw error
    }
  }
