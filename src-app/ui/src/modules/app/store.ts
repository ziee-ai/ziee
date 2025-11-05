import { create } from 'zustand'
import { ApiClient } from '../../api-client'
import type { SetupAdminRequest } from '../../api-client/types'
import type { StoreProxy } from '@/core/stores'

interface AppState {
  needsSetup: boolean | null
  isCheckingSetup: boolean
  isSettingUpAdmin: boolean
  setupError: string | null
}

// Augment the RegisteredStores interface for IntelliSense
declare module '../../core/stores' {
  interface RegisteredStores {
    App: StoreProxy<AppState>
  }
}

const defaultState: AppState = {
  needsSetup: null,
  isCheckingSetup: false,
  isSettingUpAdmin: false,
  setupError: null,
}

export const useAppStore = create<AppState>(() => defaultState)

// App actions
export const checkSetupStatus = async (): Promise<void> => {
  const state = useAppStore.getState()
  if (state.isCheckingSetup) {
    return
  }

  useAppStore.setState({ isCheckingSetup: true })

  try {
    const response = await ApiClient.App.getSetupStatus(undefined, undefined)
    useAppStore.setState({
      needsSetup: response.needs_setup,
      isCheckingSetup: false,
    })
  } catch (error) {
    console.error('Failed to check setup status:', error)
    // If we can't check setup status, assume it's not needed
    useAppStore.setState({
      needsSetup: false,
      isCheckingSetup: false,
    })
  }
}

export const setupAdmin = async (request: SetupAdminRequest): Promise<void> => {
  const state = useAppStore.getState()
  if (state.isSettingUpAdmin) {
    return
  }

  useAppStore.setState({ isSettingUpAdmin: true, setupError: null })

  try {
    await ApiClient.App.setupAdmin(request, undefined)
    useAppStore.setState({
      isSettingUpAdmin: false,
      needsSetup: false,
      setupError: null,
    })
  } catch (error: any) {
    const message =
      error?.response?.data?.message ||
      error?.message ||
      'Setup failed. Please try again.'
    useAppStore.setState({
      isSettingUpAdmin: false,
      setupError: message,
    })
    throw error
  }
}

export const clearSetupError = (): void => {
  useAppStore.setState({ setupError: null })
}
