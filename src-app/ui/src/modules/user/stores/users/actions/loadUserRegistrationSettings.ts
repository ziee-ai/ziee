import type { UsersSet, UsersGet } from '../state'

export default (set: UsersSet, get: UsersGet) => async (): Promise<void> => {
  const state = get()
  if (state.registrationSettingsInitialized || state.loadingRegistrationSettings) return
  try {
    set({ loadingRegistrationSettings: true, error: null })
    // TODO: Replace with actual API call when backend endpoint exists.
    set({
      userRegistrationEnabled: true, // Default for now
      registrationSettingsInitialized: true,
      loadingRegistrationSettings: false,
    })
  } catch (error) {
    set({
      error:
        error instanceof Error ? error.message : 'Failed to load registration settings',
      loadingRegistrationSettings: false,
    })
    throw error
  }
}
