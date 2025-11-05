import type { StoreProxy } from '@/core/stores'

interface LayoutUIState {
  // Mobile/responsive state
  isMobile: boolean
  isFullscreen: boolean
  mobileOverlayOpen: boolean
  // Sidebar state
  isSidebarCollapsed: boolean
  mainContentWidth: number
}

declare module '@/core/stores' {
  interface RegisteredStores {
    AppLayout: StoreProxy<LayoutUIState>
  }
}
