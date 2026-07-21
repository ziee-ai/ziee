import { ApiClient } from '@/api-client'
import type { SetupAdminRequest } from '@/api-client/types'
import type { AppSet, AppGet } from '../state'

export default (set: AppSet, get: AppGet) => async (request: SetupAdminRequest) => {
  if (get().isSettingUpAdmin) return
  set({ isSettingUpAdmin: true, setupError: null })
  try {
    await ApiClient.App.setupAdmin(request, undefined)
    set({ isSettingUpAdmin: false, needsSetup: false, setupError: null })
  } catch (error: any) {
    const message =
      error?.response?.data?.message || error?.message || 'Setup failed. Please try again.'
    set({ isSettingUpAdmin: false, setupError: message })
    throw error
  }
}
