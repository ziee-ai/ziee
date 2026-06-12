import { create } from 'zustand'
import { subscribeWithSelector } from 'zustand/middleware'
import { immer } from 'zustand/middleware/immer'

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

  // Actions
  setIsMobile: (isMobile: boolean) => void
  setMobileOverlayOpen: (open: boolean) => void
  closeMobileOverlay: () => void
  setSidebarCollapsed: (collapsed: boolean) => void
  toggleSidebar: () => void
  setSidebarWidth: (width: number) => void
  setMainContentWidth: (width: number) => void
  setIsFullscreen: (isFullscreen: boolean) => void
}

export const useAppLayoutStore = create<AppLayoutState>()(
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
      }),
    ),
  ),
)
