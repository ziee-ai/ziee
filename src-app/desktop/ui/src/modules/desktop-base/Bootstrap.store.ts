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

  setStatus: (status: BootstrapStatus, message?: string | null) => void
  setAttempt: (attempt: number) => void
  reset: () => void
}

export const useBootstrapStore = create<BootstrapState>(set => ({
  status: 'idle',
  attempt: 0,
  message: null,

  setStatus: (status, message = null) => set({ status, message }),
  setAttempt: attempt => set({ attempt }),
  reset: () => set({ status: 'idle', attempt: 0, message: null }),
}))
