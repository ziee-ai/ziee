import type { StoreSet } from '@ziee/framework/store-kit'
import type { HostMountPolicyResponse } from '@/api-client/types'

export const hostMountPolicyState = {
  policy: null as HostMountPolicyResponse | null,
  loading: false,
  saving: false,
  error: null as string | null,
}

export type HostMountPolicyState = typeof hostMountPolicyState
export type HostMountPolicySet = StoreSet<HostMountPolicyState>
export type HostMountPolicyGet = () => HostMountPolicyState
