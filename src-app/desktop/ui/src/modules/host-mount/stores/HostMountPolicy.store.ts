// Host-mount policy store (desktop bundle, admin).
//
// Singleton deployment policy: enabled / allowed_prefixes / allow_readwrite,
// via GET+PUT /api/host-mounts/policy.

import { ApiClient } from '@/api-client'
import type {
  HostMountPolicyResponse,
  UpdateHostMountPolicyRequest,
} from '@/api-client/types'
import { defineStore } from '@ziee/framework/store-kit'
import { type StoreProxy } from '@ziee/framework/stores'

interface HostMountPolicyState {
  policy: HostMountPolicyResponse | null
  loading: boolean
  saving: boolean
  error: string | null
  loadPolicy: () => Promise<void>
  updatePolicy: (patch: UpdateHostMountPolicyRequest) => Promise<void>
}

declare module '@ziee/framework/stores' {
  interface RegisteredStores {
    HostMountPolicy: StoreProxy<HostMountPolicyState>
  }
}

export const HostMountPolicy = defineStore('HostMountPolicy', {
  immer: true,
  state: {
    policy: null as HostMountPolicyResponse | null,
    loading: false,
    saving: false,
    error: null as string | null,
  },
  actions: set => ({
    loadPolicy: async () => {
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
    },
    updatePolicy: async (patch: UpdateHostMountPolicyRequest) => {
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
    },
  }),
  // Eager-load so the settings page renders with real data on first mount.
  init: ({ actions }) => {
    void actions.loadPolicy()
  },
})

export const useHostMountPolicyStore = HostMountPolicy.store
