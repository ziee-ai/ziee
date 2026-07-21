import type { HardwareGet, HardwareSet } from '../state'
import { recordDisconnectTime, resetSseGuards } from './_sseConnect'

export default (set: HardwareSet, _get: HardwareGet) =>
  async () => {
    console.log('Disconnecting hardware usage monitoring')
    recordDisconnectTime()
    resetSseGuards()
    set({
      sseConnected: false,
      sseConnecting: false,
      sseError: null,
      currentUsage: null,
      usageLoading: false,
    })
  }
