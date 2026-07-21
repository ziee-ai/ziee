import type { HardwareGet, HardwareSet } from '../state'

export default (set: HardwareSet, _get: HardwareGet) =>
  async () => {
    set({ hardwareError: null, usageError: null, sseError: null })
  }
