import type { StoreSet } from '@ziee/framework/store-kit'

export type BootstrapStatus = 'idle' | 'retrying' | 'succeeded' | 'failed'

export const bootstrapState = {
  status: 'idle' as BootstrapStatus,
  attempt: 0,
  message: null as string | null,
}

export type BootstrapState = typeof bootstrapState
export type BootstrapSet = StoreSet<BootstrapState>
export type BootstrapGet = () => BootstrapState
