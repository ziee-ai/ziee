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
  mainContentWidth: number

  // Actions
  setIsMobile: (isMobile: boolean) => void
  setMobileOverlayOpen: (open: boolean) => void
  closeMobileOverlay: () => void
  setSidebarCollapsed: (collapsed: boolean) => void
  toggleSidebar: () => void
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
