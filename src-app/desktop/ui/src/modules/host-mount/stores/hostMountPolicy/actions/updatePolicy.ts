import { ApiClient } from '@/api-client'
import type { HostMountPolicyGet, HostMountPolicySet } from '../state'
import type { UpdateHostMountPolicyRequest } from '@/api-client/types'

export default (set: HostMountPolicySet, _get: HostMountPolicyGet) =>
  async (patch: UpdateHostMountPolicyRequest) => {
    try {
      set({ saving: true, error: null })
      const policy = await ApiClient.HostMount.updatePolicy(patch)
      set({ policy, saving: false })
    } catch (error) {
      set({
        error: error instanceof Error ? error.message : 'Failed to save policy',
        saving: false,
      })
      throw error
    }
  }
