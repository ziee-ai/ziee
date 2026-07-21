import { ApiClient } from '@/api-client'
import type { HardwareGet, HardwareSet } from '../state'

export default (set: HardwareSet, get: HardwareGet) =>
  async () => {
    const state = get()
    if (state.hardwareInitialized || state.hardwareLoading) return
    set({ hardwareLoading: true, hardwareError: null })
    try {
      const response = await ApiClient.Hardware.info(undefined)
      set({
        hardwareInfo: response.hardware,
        hardwareInitialized: true,
        hardwareLoading: false,
        hardwareError: null,
      })
    } catch (error) {
      console.error('Hardware info loading failed:', error)
      set({
        hardwareLoading: false,
        hardwareError: error instanceof Error ? error.message : 'Unknown error',
        hardwareInitialized: false,
      })
      throw error
    }
  }
