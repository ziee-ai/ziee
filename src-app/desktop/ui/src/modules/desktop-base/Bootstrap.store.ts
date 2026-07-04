/**
 * Desktop Bootstrap Store — tracks the auto-login retry state so the desktop
 * AuthGuard can render a meaningful spinner caption. Init is driven by the
 * module's `initialize` lifecycle (auto-login retry loop), so there's no eager
 * load here; the store just resets on destroy.
 */
import { defineStore } from '@/core/store-kit'

export type BootstrapStatus = 'idle' | 'retrying' | 'succeeded' | 'failed'

export const Bootstrap = defineStore('Bootstrap', {
  state: {
    status: 'idle' as BootstrapStatus,
    attempt: 0,
    message: null as string | null,
  },
  actions: set => ({
    setStatus: (status: BootstrapStatus, message: string | null = null) =>
      set({ status, message }),
    setAttempt: (attempt: number) => set({ attempt }),
    reset: () => set({ status: 'idle', attempt: 0, message: null }),
  }),
  init: ({ actions, onCleanup }) => {
    onCleanup(() => actions.reset())
  },
})

export const useBootstrapStore = Bootstrap.store
