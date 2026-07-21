import { ApiClient } from '@/api-client'
import type { AppSet, AppGet } from '../state'

export default (set: AppSet, get: AppGet) => async () => {
  if (get().isCheckingSetup) return
  set({ isCheckingSetup: true })
  try {
    const response = await ApiClient.App.getSetupStatus(undefined, undefined)
    set({ needsSetup: response.needs_setup, isCheckingSetup: false })
  } catch (error) {
    console.error('Failed to check setup status:', error)
    // If we can't check, assume setup is needed (safe default).
    set({ needsSetup: true, isCheckingSetup: false })
  }
}
