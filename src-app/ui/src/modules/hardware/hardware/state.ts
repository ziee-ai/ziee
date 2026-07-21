import type { StoreSet } from '@ziee/framework/store-kit'
import type { HardwareInfo, HardwareUsageUpdate } from '@/api-client/types'

export const hardwareState = {
  // Static hardware information
  hardwareInfo: null as HardwareInfo | null,
  hardwareLoading: false,
  hardwareError: null as string | null,
  hardwareInitialized: false,
  // Real-time usage data
  currentUsage: null as HardwareUsageUpdate | null,
  usageLoading: false,
  usageError: null as string | null,
  // SSE connection state
  sseConnected: false,
  sseConnecting: false,
  sseError: null as string | null,
}

export type HardwareState = typeof hardwareState
export type HardwareSet = StoreSet<HardwareState>
export type HardwareGet = () => HardwareState
