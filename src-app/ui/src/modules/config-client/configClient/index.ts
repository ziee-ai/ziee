import { createJSONStorage } from 'zustand/middleware'
import { defineStore, registerLazyStore } from '@ziee/framework/store-kit'
import { configClientState, type ConfigClientState, type ThemePreference } from './state'
import type { AccentPreset } from '@/components/ThemeProvider/accentPresets'
import { ACCENT_PRESETS, DEFAULT_ACCENT } from '@/components/ThemeProvider/accentPresets'
import type { Actions } from './actions.gen'

// Guarded persistence storage. Accessing `localStorage` throws in locked-down
// contexts (private-mode quota, disabled storage, sandboxed iframe). Probe once
// and fall back to in-memory so store creation never takes the app down; the
// preference simply won't survive a reload there.
const safeStorage = createJSONStorage(() => {
  try {
    const probe = '__ziee_ls_probe__'
    window.localStorage.setItem(probe, probe)
    window.localStorage.removeItem(probe)
    return window.localStorage
  } catch {
    const mem = new Map<string, string>()
    return {
      getItem: (name: string) => mem.get(name) ?? null,
      setItem: (name: string, value: string) => {
        mem.set(name, value)
      },
      removeItem: (name: string) => {
        mem.delete(name)
      },
    }
  }
})

// Guard against a stale persisted accent id that no longer exists in code.
const normalizeAccent = (a: AccentPreset): AccentPreset =>
  a in ACCENT_PRESETS ? a : DEFAULT_ACCENT

const ConfigClientDef = defineStore<ConfigClientState, Actions>('ConfigClient', {
  persist: {
    name: 'config-client-storage',
    storage: safeStorage,
    partialize: state => ({
      themePreference: state.themePreference,
      accentPreset: state.accentPreset,
    }),
    merge: (persisted, current) => {
      const p = (persisted ?? {}) as {
        themePreference?: ThemePreference
        accentPreset?: AccentPreset
      }
      return {
        ...current,
        ...p,
        accentPreset: normalizeAccent(p.accentPreset ?? current.accentPreset),
      }
    },
  },
  state: configClientState,
  actions: import.meta.glob('./actions/*.ts'),
})

export const ConfigClient = registerLazyStore(ConfigClientDef)
export const useConfigClientStore = ConfigClientDef.store

export type { ThemePreference } from './state'

declare module '@ziee/framework/stores' {
  interface RegisteredStores {
    ConfigClient: import('@ziee/framework/stores').StoreProxy<ReturnType<typeof ConfigClientDef.store.getState>>
  }
}
