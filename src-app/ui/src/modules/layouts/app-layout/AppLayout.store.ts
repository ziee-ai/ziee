import { create } from 'zustand'
import {
  createJSONStorage,
  persist,
  subscribeWithSelector,
} from 'zustand/middleware'
import { immer } from 'zustand/middleware/immer'

// Guarded persistence storage (mirrors ConfigClient.store): accessing
// localStorage throws in locked-down contexts (private mode, sandboxed
// iframe). Probe once and fall back to in-memory so store creation never
// takes the app down; the preference just won't survive a reload there.
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

interface AppLayoutState {
  // Mobile/responsive state
  isMobile: boolean
  isFullscreen: boolean
  mobileOverlayOpen: boolean

  // Sidebar state
  isSidebarCollapsed: boolean
  /**
   * Sidebar width in px. Persisted in the store (NOT a per-mount ref)
   * because each route's `*Layout` component mounts its own
   * `<AppLayout>` instance — a `useRef` would reset to its initial
   * value every time the user navigates between Chat / Settings /
   * etc., wiping the user's resized width. Read at AppLayout mount
   * time, written on drag end.
   */
  sidebarWidth: number
  mainContentWidth: number

  /**
   * When true, the app shell unwinds its fixed-height/overflow clamp so the
   * DOCUMENT (window) scrolls instead of an inner scroller — the condition iOS
   * Safari needs to collapse its toolbar + flow content under the notch. Set
   * only by pages that opt in via `useNativeScroll` (Settings, mobile only);
   * defaults off so every other page keeps the fixed shell. Ephemeral (not
   * persisted).
   */
  nativeScroll: boolean

  /**
   * In native scroll mode, whether the auto-hiding header is currently hidden
   * (scrolled away on swipe-up). Shared so the fixed sidebar-toggle button can
   * disappear/reappear together with the header. Ephemeral.
   */
  headerHidden: boolean

  // Actions
  setIsMobile: (isMobile: boolean) => void
  setMobileOverlayOpen: (open: boolean) => void
  closeMobileOverlay: () => void
  setSidebarCollapsed: (collapsed: boolean) => void
  toggleSidebar: () => void
  setSidebarWidth: (width: number) => void
  setMainContentWidth: (width: number) => void
  setIsFullscreen: (isFullscreen: boolean) => void
  setNativeScroll: (nativeScroll: boolean) => void
  setHeaderHidden: (headerHidden: boolean) => void
}

export const useAppLayoutStore = create<AppLayoutState>()(
  persist(
    subscribeWithSelector(
      immer(
        (set, get): AppLayoutState => ({
        // Initial state
        isMobile: false,
        isFullscreen: false,
        mobileOverlayOpen: false,
        isSidebarCollapsed: false,
        sidebarWidth: 240,
        mainContentWidth: 1000,
        nativeScroll: false,
        headerHidden: false,

        // Actions
        setIsMobile: (isMobile: boolean) => {
          set({ isMobile })
        },

        setMobileOverlayOpen: (open: boolean) => {
          set({ mobileOverlayOpen: open })
        },

        closeMobileOverlay: () => {
          set({ mobileOverlayOpen: false })
        },

        setSidebarCollapsed: (collapsed: boolean) => {
          set({ isSidebarCollapsed: collapsed })
        },

        toggleSidebar: () => {
          const currentState = get()
          set({ isSidebarCollapsed: !currentState.isSidebarCollapsed })
        },

        setSidebarWidth: (width: number) => {
          set({ sidebarWidth: width })
        },

        setMainContentWidth: (width: number) => {
          set({ mainContentWidth: width })
        },

        setIsFullscreen: (isFullscreen: boolean) => {
          set({ isFullscreen })
        },

        setNativeScroll: (nativeScroll: boolean) => {
          set({ nativeScroll })
        },

        setHeaderHidden: (headerHidden: boolean) => {
          set({ headerHidden })
        },
        }),
      ),
    ),
    {
      name: 'app-layout-storage',
      storage: safeStorage,
      // Persist ONLY the sidebar collapse preference so it survives a reload
      // (a common UX expectation). Ephemeral/derived layout state — isMobile,
      // overlay open, fullscreen, measured widths — must NOT persist; they are
      // recomputed from the viewport on each load.
      partialize: state => ({ isSidebarCollapsed: state.isSidebarCollapsed }),
    },
  ),
)
