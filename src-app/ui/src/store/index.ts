// Re-export layout store for compatibility with useWindowMinSize hook
export {
  useAppLayoutStore,
  setIsMobile,
  setMobileOverlayOpen,
  closeMobileOverlay,
  setSidebarCollapsed,
  toggleSidebar,
  setMainContentWidth,
  setIsFullscreen,
} from '../components/Layout/appLayoutStore'
