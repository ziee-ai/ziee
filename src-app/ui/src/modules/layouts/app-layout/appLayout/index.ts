import { defineStore, registerLazyStore } from '@ziee/framework/store-kit'
import { appLayoutSeam } from '@ziee/shell'
import { createJSONStorage } from 'zustand/middleware'
import { appLayoutState, type AppLayoutState } from './state'
import type { Actions } from './actions.gen'

// Guarded persistence storage (mirrors ConfigClient.store): accessing
// localStorage throws in locked-down contexts (private mode, sandboxed iframe).
// Probe once and fall back to in-memory so store creation never takes the app
// down; the preference just won't survive a reload there.
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

export const AppLayoutDef = defineStore<AppLayoutState, Actions>('AppLayout', {
  persist: {
    name: 'app-layout-storage',
    storage: safeStorage,
    // Persist ONLY the sidebar collapse preference so it survives a reload.
    // Ephemeral/derived layout state (isMobile, overlay, fullscreen, measured
    // widths) must NOT persist — they're recomputed from the viewport on load.
    partialize: state => ({ isSidebarCollapsed: state.isSidebarCollapsed }),
  },
  state: appLayoutState,
  actions: import.meta.glob('./actions/*.ts'),
})

export const AppLayout = registerLazyStore(AppLayoutDef)
export const useAppLayoutStore = AppLayoutDef.store

// SEAM: inject into the SDK shell (replaces the old global Stores.AppLayout).
appLayoutSeam.set(AppLayout)
