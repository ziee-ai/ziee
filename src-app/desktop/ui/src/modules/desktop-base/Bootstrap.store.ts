/**
 * Desktop Bootstrap Store
 *
 * Tracks the auto-login retry state so the desktop AuthGuard can render
 * a meaningful spinner caption instead of staring at a generic spinner
 * forever when the embedded server is slow to start (or fails).
 */

import { create } from 'zustand'

export type BootstrapStatus = 'idle' | 'retrying' | 'succeeded' | 'failed'

interface BootstrapState {
  status: BootstrapStatus
  attempt: number
  message: string | null

  __init__: {
    /** Fires on first property access through the store proxy. Bootstrap
     *  init is driven by the module's `initialize` lifecycle, so this is
     *  a no-op that satisfies the proxy's lifecycle contract. */
    __store__: () => void
  }
  __destroy__: () => void

  setStatus: (status: BootstrapStatus, message?: string | null) => void
  setAttempt: (attempt: number) => void
  reset: () => void
}

export const useBootstrapStore = create<BootstrapState>((set, get) => ({
  status: 'idle',
  attempt: 0,
  message: null,

  __init__: {
    __store__: () => {
      // Bootstrap init is handled by the module's `initialize` lifecycle
      // (auto-login retry loop), so no eager load is needed here.
    },
  },

  __destroy__: () => {
    get().reset()
  },

  setStatus: (status, message = null) => set({ status, message }),
  setAttempt: attempt => set({ attempt }),
  reset: () => set({ status: 'idle', attempt: 0, message: null }),
}))
