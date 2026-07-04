import { createJSONStorage } from 'zustand/middleware'
import {
  type AccentPreset,
  ACCENT_PRESETS,
  DEFAULT_ACCENT,
} from '@/components/ThemeProvider/accentPresets'
import { defineStore } from '@/core/store-kit'
import type { StoreProxy } from '@/core/stores'

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

export type ThemePreference = 'light' | 'dark' | 'system'

// Guard against a stale persisted accent id that no longer exists in code.
const normalizeAccent = (a: AccentPreset): AccentPreset =>
  a in ACCENT_PRESETS ? a : DEFAULT_ACCENT

export const ConfigClient = defineStore('ConfigClient', {
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
  state: {
    themePreference: 'system' as ThemePreference,
    /** User-selected brand accent (Settings → Appearance). Drives --primary/--ring. */
    accentPreset: DEFAULT_ACCENT as AccentPreset,
  },
  actions: (set, get) => ({
    setThemePreference: (preference: ThemePreference) => {
      set({ themePreference: preference })
    },
    getThemePreference: (): ThemePreference => get().themePreference,
    setAccentPreset: (preset: AccentPreset) => {
      set({ accentPreset: normalizeAccent(preset) })
    },
  }),
})

export const useConfigClientStore = ConfigClient.store

declare module '../../core/stores' {
  interface RegisteredStores {
    ConfigClient: StoreProxy<ReturnType<typeof ConfigClient.store.getState>>
  }
}
