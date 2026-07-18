import { createJSONStorage } from 'zustand/middleware'
import { defineStore } from '@ziee/framework/store-kit'

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

export const AppLayout = defineStore('AppLayout', {
  persist: {
    name: 'app-layout-storage',
    storage: safeStorage,
    // Persist ONLY the sidebar collapse preference so it survives a reload.
    // Ephemeral/derived layout state (isMobile, overlay, fullscreen, measured
    // widths) must NOT persist — they're recomputed from the viewport on load.
    partialize: state => ({ isSidebarCollapsed: state.isSidebarCollapsed }),
  },
  state: {
    // Mobile/responsive state
    isMobile: false,
    isFullscreen: false,
    mobileOverlayOpen: false,
    // Sidebar state
    isSidebarCollapsed: false,
    /**
     * Sidebar width in px. Persisted in the store (NOT a per-mount ref) because
     * each route's `*Layout` mounts its own `<AppLayout>` — a `useRef` would
     * reset on every navigation, wiping the user's resized width.
     */
    sidebarWidth: 240,
    mainContentWidth: 1000,
    /**
     * When true, the app shell unwinds its fixed-height/overflow clamp so the
     * DOCUMENT scrolls instead of an inner scroller (iOS Safari toolbar collapse
     * + content under the notch). Set only by pages that opt in via
     * `useNativeScroll` (Settings, mobile only). Ephemeral (not persisted).
     */
    nativeScroll: false,
    /**
     * In native scroll mode, whether the auto-hiding header is currently hidden.
     * Shared so the fixed sidebar-toggle button disappears/reappears with the
     * header. Ephemeral.
     */
    headerHidden: false,
  },
  actions: (set, get) => ({
    setIsMobile: (isMobile: boolean) => set({ isMobile }),
    setMobileOverlayOpen: (open: boolean) => set({ mobileOverlayOpen: open }),
    closeMobileOverlay: () => set({ mobileOverlayOpen: false }),
    setSidebarCollapsed: (collapsed: boolean) => set({ isSidebarCollapsed: collapsed }),
    toggleSidebar: () => set({ isSidebarCollapsed: !get().isSidebarCollapsed }),
    setSidebarWidth: (width: number) => set({ sidebarWidth: width }),
    setMainContentWidth: (width: number) => set({ mainContentWidth: width }),
    setIsFullscreen: (isFullscreen: boolean) => set({ isFullscreen }),
    setNativeScroll: (nativeScroll: boolean) => set({ nativeScroll }),
    setHeaderHidden: (headerHidden: boolean) => set({ headerHidden }),
  }),
})

export const useAppLayoutStore = AppLayout.store
