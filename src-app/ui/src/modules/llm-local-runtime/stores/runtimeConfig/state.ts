import type { GpuDetectionResponse, RuntimeSettings } from '@/api-client/types'
import type { StoreSet } from '@ziee/framework/store-kit'

export const runtimeConfigState = {
  // Singleton runtime settings (idle / auto-start / drain / allow_unsigned)
  settings: null as RuntimeSettings | null,
  loadingSettings: false,
  savingSettings: false,
  // GPU detection result (powers the GPU card)
  gpu: null as GpuDetectionResponse | null,
  loadingGpu: false,
  error: null as string | null,
}

export type RuntimeConfigState = typeof runtimeConfigState
export type RuntimeConfigSet = StoreSet<RuntimeConfigState>
export type RuntimeConfigGet = () => RuntimeConfigState
