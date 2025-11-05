import { create } from 'zustand'

interface LayoutUIState {
  // Mobile/responsive state
  isMobile: boolean
  isFullscreen: boolean
  mobileOverlayOpen: boolean
  // Sidebar state
  isSidebarCollapsed: boolean
  mainContentWidth: number
}

export const useAppLayoutStore = create<LayoutUIState>(() => ({
  // Initial state
  isMobile: false,
  isFullscreen: false,
  mobileOverlayOpen: false,
  isSidebarCollapsed: false,
  mainContentWidth: 1000,
}))

// Actions
export const setIsMobile = (isMobile: boolean) => {
  useAppLayoutStore.setState({ isMobile })
}

export const setMobileOverlayOpen = (open: boolean) => {
  useAppLayoutStore.setState({ mobileOverlayOpen: open })
}

export const closeMobileOverlay = () => {
  useAppLayoutStore.setState({ mobileOverlayOpen: false })
}

export const setSidebarCollapsed = (collapsed: boolean) => {
  useAppLayoutStore.setState({ isSidebarCollapsed: collapsed })
}

export const toggleSidebar = () => {
  const currentState = useAppLayoutStore.getState()
  useAppLayoutStore.setState({
    isSidebarCollapsed: !currentState.isSidebarCollapsed,
  })
}

export const setMainContentWidth = (width: number) => {
  useAppLayoutStore.setState({ mainContentWidth: width })
}

export const setIsFullscreen = (isFullscreen: boolean) => {
  useAppLayoutStore.setState({ isFullscreen })
}
