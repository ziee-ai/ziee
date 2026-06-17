// Host-mount policy store (desktop bundle, admin).
//
// Singleton deployment policy: enabled / allowed_prefixes / allow_readwrite,
// via GET+PUT /api/host-mounts/policy. Mirrors RemoteAccess.store's eager-load.

import { create } from 'zustand'
import { subscribeWithSelector } from 'zustand/middleware'
import { immer } from 'zustand/middleware/immer'

import { ApiClient } from '@/api-client'
import type {
  HostMountPolicyResponse,
  UpdateHostMountPolicyRequest,
} from '@/api-client/types'
import { type StoreProxy } from '@/core/stores'

interface HostMountPolicyState {
  policy: HostMountPolicyResponse | null
  loading: boolean
  saving: boolean
  error: string | null

  __init__: {
    // `__store__` fires on first access of ANY store property (the eager-load
    // idiom). A named key like `load` would only fire when `.load` itself is
    // read — which the page never does (it reads `.policy`), so the policy
    // would never load. See core/stores.ts:242.
    __store__: () => Promise<void>
  }

  loadPolicy: () => Promise<void>
  updatePolicy: (patch: UpdateHostMountPolicyRequest) => Promise<void>
}

declare module '@/core/stores' {
  interface RegisteredStores {
    HostMountPolicy: StoreProxy<HostMountPolicyState>
  }
}

export const useHostMountPolicyStore = create<HostMountPolicyState>()(
  subscribeWithSelector(
    immer((set, get): HostMountPolicyState => ({
      policy: null,
      loading: false,
      saving: false,
      error: null,

      __init__: {
        // Eager-load so the settings page renders with real data on first mount.
        __store__: async () => {
          await get().loadPolicy()
        },
      },

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

      updatePolicy: async (patch) => {
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
    })),
  ),
)
