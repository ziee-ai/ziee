import { ApiClient } from '@/api-client'
import type { HostMountPolicyGet, HostMountPolicySet } from '../state'

export default (set: HostMountPolicySet, _get: HostMountPolicyGet) =>
  async () => {
    try {
      set({ loading: true, error: null })
      const policy = await ApiClient.HostMount.getPolicy()
      set({ policy, loading: false })
    } catch (error) {
      set({
        error: error instanceof Error ? error.message : 'Failed to load policy',
        loading: false,
      })
    }
  }
